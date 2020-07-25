use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, ProfileEmbed},
    scraper::OsuStatsParams,
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    BotResult, Context,
};

use rosu::{
    backend::requests::UserRequest,
    models::{Beatmap, GameMode, GameMods, Score, User},
};
use std::{
    cmp::Ordering::Equal,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use twilight::model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn profile_send(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args,
) -> BotResult<()> {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieve the user and its top scores
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let user = match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    let content = format!("User `{}` was not found", name);
                    msg.respond(&ctx, content).await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        let scores = match user.get_top_scores(&osu, 100, mode).await {
            Ok(scores) => scores,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        (user, scores)
    };

    let (profile_result, missing_maps, retrieving_msg, globals_count) = match tokio::try_join!(
        process_maps(ctx, mode, scores, msg.channel_id),
        get_globals_count(ctx, user.username.clone(), mode)
    ) {
        Ok(((profile_result, missing_maps, retrieving_msg), globals_count)) => {
            (profile_result, missing_maps, retrieving_msg, globals_count)
        }
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why);
        }
    };

    // Accumulate all necessary data
    let data = ProfileEmbed::new(user, profile_result, globals_count, &ctx.cache).await;

    if let Some(msg) = retrieving_msg {
        msg.delete(ctx).await?;
    }

    // Send the embed
    let response = msg
        .channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await;

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        let len = maps.len();
        match mysql.insert_beatmaps(&maps).await {
            Ok(_) if len == 1 => {}
            Ok(_) => info!("Added {} maps to DB", len),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }
    response?.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}

async fn process_maps(
    ctx: &Context,
    mode: GameMode,
    scores: Vec<Score>,
    channel: ChannelId,
) -> Result<(Option<ProfileResult>, Option<Vec<Beatmap>>, Option<Message>), CommandError> {
    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores.iter().map(|s| s.beatmap_id.unwrap()).collect();
    let mut maps = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        mysql
            .get_beatmaps(&map_ids)
            .await
            .unwrap_or_else(|_| HashMap::default())
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());
    let retrieving_msg = if scores.len() - maps.len() > 15 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );
        channel.say(ctx, content).await.ok()
    } else {
        None
    };
    // Retrieving all missing beatmaps
    let mut score_maps = Vec::with_capacity(scores.len());
    let mut missing_indices = Vec::with_capacity(scores.len() / 2);
    {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        for (i, score) in scores.into_iter().enumerate() {
            let map_id = score.beatmap_id.unwrap();
            let map = if maps.contains_key(&map_id) {
                maps.remove(&map_id).unwrap()
            } else {
                missing_indices.push(i);
                score.get_beatmap(osu).await?
            };
            score_maps.push((score, map));
        }
    }
    let missing_maps: Option<Vec<Beatmap>> = if missing_indices.is_empty() {
        None
    } else {
        Some(
            score_maps
                .par_iter()
                .enumerate()
                .filter(|(i, _)| missing_indices.contains(i))
                .map(|(_, (_, map))| map.clone())
                .collect(),
        )
    };
    let profile_result = if score_maps.is_empty() {
        None
    } else {
        Some(ProfileResult::calc(mode, score_maps))
    };
    Ok((profile_result, missing_maps, retrieving_msg))
}

async fn get_globals_count(
    ctx: &Context,
    name: String,
    mode: GameMode,
) -> Result<BTreeMap<usize, String>, CommandError> {
    let data = ctx.data.read().await;
    let scraper = data.get::<Scraper>().unwrap();
    let mut counts = BTreeMap::new();
    let mut params = OsuStatsParams::new(name).mode(mode);
    let mut get_amount = true;
    for rank in [50, 25, 15, 8, 1].iter() {
        if !get_amount {
            counts.insert(*rank, 0);
            continue;
        }
        params = params.rank_max(*rank);
        match scraper.get_global_scores(&params).await {
            Ok((_, count)) => {
                counts.insert(*rank, numbers::with_comma_u64(count as u64));
                if count == 0 {
                    get_amount = false;
                }
            }
            Err(why) => error!("Error while retrieving osustats for profile: {}", why),
        }
    }
    Ok(counts)
}

#[command]
#[short_desc("Display statistics of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osu")]
pub async fn profile(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    profile_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a mania user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mania", "maniaprofile", "profilem")]
pub async fn profilemania(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    profile_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a taiko user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("taiko", "taikoprofile", "profilet")]
pub async fn profiletaiko(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    profile_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of ctb user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("ctb", "ctbprofile", "profilec")]
pub async fn profilectb(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    profile_send(GameMode::CTB, ctx, msg, args).await
}

pub struct ProfileResult {
    pub mode: GameMode,

    pub min_acc: f32,
    pub max_acc: f32,
    pub avg_acc: f32,

    pub min_pp: f32,
    pub max_pp: f32,
    pub avg_pp: f32,

    pub min_combo: u32,
    pub max_combo: u32,
    pub avg_combo: u32,
    pub map_combo: u32,

    pub min_len: u32,
    pub max_len: u32,
    pub avg_len: u32,

    pub mappers: Vec<(String, u32, f32)>,

    pub mod_combs_count: Option<Vec<(GameMods, u32)>>,
    pub mod_combs_pp: Option<Vec<(GameMods, f32)>>,
    pub mods_count: Vec<(GameMods, u32)>,
    pub mods_pp: Vec<(GameMods, f32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, tuples: Vec<(Score, Beatmap)>) -> Self {
        let (mut min_acc, mut max_acc, mut avg_acc) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_pp, mut max_pp, mut avg_pp) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_combo, mut max_combo, mut avg_combo, mut map_combo) = (u32::MAX, 0, 0, 0);
        let (mut min_len, mut max_len, mut avg_len) = (f32::MAX, 0.0_f32, 0.0);
        let len = tuples.len() as f32;
        let mut mappers = HashMap::with_capacity(len as usize);
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut factor = 1.0;
        let mut mult_mods = false;
        for (score, map) in tuples {
            let acc = score.accuracy(mode);
            min_acc = min_acc.min(acc);
            max_acc = max_acc.max(acc);
            avg_acc += acc;

            if let Some(pp) = score.pp {
                min_pp = min_pp.min(pp);
                max_pp = max_pp.max(pp);
                avg_pp += pp;
            }

            min_combo = min_combo.min(score.max_combo);
            max_combo = max_combo.max(score.max_combo);
            avg_combo += score.max_combo;

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

            min_len = min_len.min(seconds_drain);
            max_len = max_len.max(seconds_drain);
            avg_len += seconds_drain;

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
                let mut nm = mods.entry(GameMods::NoMod).or_insert((0, 0.0));
                nm.0 += 1;
                nm.1 += weighted_pp;
            } else {
                mult_mods |= score.enabled_mods.len() > 1;
                for m in score.enabled_mods {
                    let mut r#mod = mods.entry(m).or_insert((0, 0.0));
                    r#mod.0 += 1;
                    r#mod.1 += weighted_pp;
                }
            }
        }
        avg_acc /= len;
        avg_pp /= len;
        avg_combo /= len as u32;
        avg_len /= len;
        map_combo /= len as u32;
        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        mods.values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        let mut mappers: Vec<_> = mappers
            .into_iter()
            .map(|(name, (count, pp))| (name, count, pp))
            .collect();
        mappers.sort_by(
            |(_, count_a, pp_a), (_, count_b, pp_b)| match count_b.cmp(&count_a) {
                Equal => pp_b.partial_cmp(pp_a).unwrap_or(Equal),
                other => other,
            },
        );
        mappers = mappers[..5.min(mappers.len())].to_vec();
        let (mod_combs_count, mod_combs_pp) = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (*name, *count))
                .collect();
            mod_combs_count.sort_by(|a, b| b.1.cmp(&a.1));
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();
            mod_combs_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
            (Some(mod_combs_count), Some(mod_combs_pp))
        } else {
            (None, None)
        };
        let mut mods_count: Vec<_> = mods
            .iter()
            .map(|(name, (count, _))| (*name, *count))
            .collect();
        mods_count.sort_by(|a, b| b.1.cmp(&a.1));
        let mut mods_pp: Vec<_> = mods
            .into_iter()
            .map(|(name, (_, avg))| (name, avg))
            .collect();
        mods_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
        Self {
            mode,
            min_acc,
            max_acc,
            avg_acc,
            min_pp,
            max_pp,
            avg_pp,
            min_combo,
            max_combo,
            avg_combo,
            map_combo,
            min_len: min_len as u32,
            max_len: max_len as u32,
            avg_len: avg_len as u32,
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
            mods_pp,
        }
    }
}
