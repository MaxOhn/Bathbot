use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, ProfileEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context, Error,
};

use futures::future::TryFutureExt;
use rayon::prelude::*;
use rosu::{
    backend::BestRequest,
    models::{Beatmap, GameMode, GameMods, Score},
};
use std::{
    cmp::{Ordering::Equal, PartialOrd},
    collections::{BTreeMap, HashMap},
    ops::{AddAssign, Div},
    sync::Arc,
};
use twilight::model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn profile_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their top scores
    let scores_fut = match BestRequest::with_username(&name) {
        Ok(req) => req.mode(GameMode::STD).limit(100).queue(ctx.osu()),
        Err(_) => {
            let content = format!("Could not build request for osu name `{}`", name);
            return msg.error(&ctx, content).await;
        }
    };
    let join_result = tokio::try_join!(
        ctx.osu_user(&name, mode).map_err(Error::Osu),
        scores_fut.map_err(Error::Osu),
    );
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why);
        }
    };
    let globals_count = match super::get_globals_count(&ctx, &user.username, mode).await {
        Ok(globals_count) => globals_count,
        Err(why) => {
            error!("Error while requesting globals count: {}", why);
            BTreeMap::new()
        }
    };

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().flat_map(|s| s.beatmap_id).collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
            HashMap::default()
        }
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());
    let retrieving_msg = if scores.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut score_maps = Vec::with_capacity(scores.len());
    let mut missing_indices = Vec::new();
    for (i, score) in scores.into_iter().enumerate() {
        let map_id = score.beatmap_id.unwrap();
        let map = if maps.contains_key(&map_id) {
            maps.remove(&map_id).unwrap()
        } else {
            missing_indices.push(i);
            score.get_beatmap(ctx.osu()).await?
        };
        score_maps.push((score, map));
    }
    let missing_maps: Option<Vec<Beatmap>> = if missing_indices.is_empty() {
        None
    } else {
        let maps = score_maps
            .par_iter()
            .enumerate()
            .filter(|(i, _)| missing_indices.contains(i))
            .map(|(_, (_, map))| map.clone())
            .collect();
        Some(maps)
    };
    let profile_result = if score_maps.is_empty() {
        None
    } else {
        Some(ProfileResult::calc(mode, score_maps))
    };

    // Accumulate all necessary data
    let data = ProfileEmbed::new(user, profile_result, globals_count);

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Send the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        match ctx.psql().insert_beatmaps(&maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }
    response.reaction_delete(&ctx, msg.author.id);
    Ok(())
}

#[command]
#[short_desc("Display statistics of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osu")]
pub async fn profile(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a mania user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mania", "maniaprofile", "profilem")]
pub async fn profilemania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a taiko user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("taiko", "taikoprofile", "profilet")]
pub async fn profiletaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a ctb user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ctb", "ctbprofile", "profilec")]
pub async fn profilectb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::CTB, ctx, msg, args).await
}

pub struct ProfileResult {
    pub mode: GameMode,

    pub acc: MinMaxAvgF32,
    pub pp: MinMaxAvgF32,
    pub map_combo: u32,
    pub combo: MinMaxAvgU32,
    pub map_len: MinMaxAvgU32,

    pub mappers: Vec<(String, u32, f32)>,
    pub mod_combs_count: Option<Vec<(GameMods, u32)>>,
    pub mod_combs_pp: Vec<(GameMods, f32)>,
    pub mods_count: Vec<(GameMods, u32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, tuples: Vec<(Score, Beatmap)>) -> Self {
        let mut acc = MinMaxAvgF32::new();
        let mut pp = MinMaxAvgF32::new();
        let mut combo = MinMaxAvgU32::new();
        let mut map_len = MinMaxAvgF32::new();
        let mut map_combo = 0;
        let mut mappers = HashMap::with_capacity(tuples.len());
        let len = tuples.len() as f32;
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut factor = 1.0;
        let mut mult_mods = false;
        for (score, map) in tuples {
            acc.add(score.accuracy(mode));
            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }
            combo.add(score.max_combo);
            if let Some(combo) = map.max_combo {
                map_combo += combo;
            }
            let seconds_drain = if score.enabled_mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.enabled_mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 1.5
            } else {
                map.seconds_drain as f32
            };
            map_len.add(seconds_drain);

            let mut mapper = mappers
                .entry(map.creator.to_lowercase())
                .or_insert((0, 0.0));
            let weighted_pp = score.pp.unwrap_or(0.0) * factor;
            factor *= 0.95;
            mapper.0 += 1;
            mapper.1 += weighted_pp;
            {
                let mut mod_comb = mod_combs.entry(score.enabled_mods).or_insert((0, 0.0));
                mod_comb.0 += 1;
                mod_comb.1 += weighted_pp;
            }
            if score.enabled_mods.is_empty() {
                *mods.entry(GameMods::NoMod).or_insert(0) += 1;
            } else {
                mult_mods |= score.enabled_mods.len() > 1;
                for m in score.enabled_mods {
                    *mods.entry(m).or_insert(0) += 1;
                }
            }
        }
        map_combo /= len as u32;
        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        mods.values_mut()
            .for_each(|count| *count = (*count as f32 * 100.0 / len) as u32);
        let mut mappers: Vec<_> = mappers
            .into_iter()
            .map(|(name, (count, pp))| (name, count, pp))
            .collect();
        mappers.sort_unstable_by(|(_, count_a, pp_a), (_, count_b, pp_b)| {
            match count_b.cmp(&count_a) {
                Equal => pp_b.partial_cmp(pp_a).unwrap_or(Equal),
                other => other,
            }
        });
        mappers = mappers[..5.min(mappers.len())].to_vec();
        let mod_combs_count = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (*name, *count))
                .collect();
            mod_combs_count.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            Some(mod_combs_count)
        } else {
            None
        };
        let mod_combs_pp = {
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();
            mod_combs_pp.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
            mod_combs_pp
        };
        let mut mods_count: Vec<_> = mods.into_iter().collect();
        mods_count.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        Self {
            mode,
            acc,
            pp,
            combo,
            map_combo,
            map_len: map_len.into(),
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
        }
    }
}

pub trait MinMaxAvgBasic {
    type Value: PartialOrd + AddAssign + Inc + Div<Output = Self::Value> + Copy;

    // Implement these
    fn new() -> Self;

    fn get(&self) -> (Self::Value, Self::Value, Self::Value, Self::Value);

    fn get_mut(
        &mut self,
    ) -> (
        &mut Self::Value,
        &mut Self::Value,
        &mut Self::Value,
        &mut Self::Value,
    );

    // Don't implement these
    fn add(&mut self, value: Self::Value) {
        let (min, max, sum, len) = self.get_mut();
        if *min > value {
            *min = value;
        }
        if *max < value {
            *max = value;
        }
        *sum += value;
        len.inc();
    }
    fn min(&self) -> Self::Value {
        let (min, _, _, _) = self.get();
        min
    }
    fn max(&self) -> Self::Value {
        let (_, max, _, _) = self.get();
        max
    }
    fn avg(&self) -> Self::Value {
        let (_, _, sum, len) = self.get();
        sum / len
    }
}

pub struct MinMaxAvgU32 {
    min: u32,
    max: u32,
    sum: u32,
    len: u32,
}

impl MinMaxAvgBasic for MinMaxAvgU32 {
    type Value = u32;
    fn new() -> Self {
        Self {
            min: u32::MAX,
            max: 0,
            sum: 0,
            len: 0,
        }
    }
    fn get(&self) -> (u32, u32, u32, u32) {
        (self.min, self.max, self.sum, self.len)
    }
    fn get_mut(&mut self) -> (&mut u32, &mut u32, &mut u32, &mut u32) {
        (&mut self.min, &mut self.max, &mut self.sum, &mut self.len)
    }
}

impl From<MinMaxAvgF32> for MinMaxAvgU32 {
    fn from(val: MinMaxAvgF32) -> Self {
        Self {
            min: val.min as u32,
            max: val.max as u32,
            sum: val.sum as u32,
            len: val.len as u32,
        }
    }
}

pub struct MinMaxAvgF32 {
    min: f32,
    max: f32,
    sum: f32,
    len: f32,
}

impl MinMaxAvgBasic for MinMaxAvgF32 {
    type Value = f32;
    fn new() -> Self {
        Self {
            min: f32::MAX,
            max: 0.0,
            sum: 0.0,
            len: 0.0,
        }
    }
    fn get(&self) -> (f32, f32, f32, f32) {
        (self.min, self.max, self.sum, self.len)
    }
    fn get_mut(&mut self) -> (&mut f32, &mut f32, &mut f32, &mut f32) {
        (&mut self.min, &mut self.max, &mut self.sum, &mut self.len)
    }
}

pub trait Inc {
    fn inc(&mut self);
}

impl Inc for f32 {
    fn inc(&mut self) {
        *self += 1.0;
    }
}

impl Inc for u32 {
    fn inc(&mut self) {
        *self += 1;
    }
}
