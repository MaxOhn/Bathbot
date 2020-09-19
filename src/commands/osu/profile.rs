use super::{MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};
use crate::{
    arguments::{Args, NameArgs},
    custom_client::{DateCount, OsuProfile},
    embeds::{EmbedData, ProfileEmbed},
    pagination::{Pagination, ProfilePagination},
    tracking::process_tracking,
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context, Error,
};

use chrono::{Date, Datelike, Utc};
use futures::future::{try_join_all, TryFutureExt};
use image::{imageops::FilterType::Lanczos3, load_from_memory, png::PngEncoder, ColorType};
use plotters::prelude::*;
use rayon::prelude::*;
use rosu::{
    backend::BestRequest,
    models::{Beatmap, GameMode, GameMods, Score},
};
use std::{
    cmp::{Ordering::Equal, PartialOrd},
    collections::{BTreeMap, HashMap},
    iter::FromIterator,
    sync::Arc,
};
use twilight_model::{
    channel::Message,
    id::{ChannelId, UserId},
};

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
    let (data, profile) =
        match profile_embed(&ctx, &name, mode, Some(msg.author.id), msg.channel_id).await? {
            Some(data) => data,
            None => return Ok(()),
        };

    // Draw the graph
    let graph = match graphs(&profile).await {
        Ok(graph_option) => graph_option,
        Err(why) => {
            warn!("Error while creating profile graph: {}", why);
            None
        }
    };

    // Send the embed
    let embed = data.build().build()?;
    let m = ctx.http.create_message(msg.channel_id).embed(embed)?;
    let response = if let Some(graph) = graph {
        m.attachment("profile_graph.png", graph).await?
    } else {
        m.await?
    };

    // Pagination
    let pagination =
        ProfilePagination::new(ctx.clone(), response, msg.channel_id, mode, name, data);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 90).await {
            warn!("Pagination error (profile): {}", why)
        }
    });
    Ok(())
}

pub async fn profile_embed(
    ctx: &Context,
    name: &str,
    mode: GameMode,
    owner: Option<UserId>,
    channel: ChannelId,
) -> BotResult<Option<(ProfileEmbed, OsuProfile)>> {
    // Retrieve the user and their top scores
    let scores_fut = match BestRequest::with_username(&name) {
        Ok(req) => req.mode(mode).limit(100).queue(ctx.osu()),
        Err(_) => {
            if let Some(owner) = owner {
                let content = format!("Could not build request for osu name `{}`", name);
                ctx.http
                    .create_message(channel)
                    .content(content)?
                    .await?
                    .reaction_delete(ctx, owner);
            }
            return Ok(None);
        }
    };
    let join_result = tokio::try_join!(
        ctx.osu_user(&name, mode).map_err(Error::Osu),
        scores_fut.map_err(Error::Osu),
    );
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            if let Some(owner) = owner {
                let content = format!("User `{}` was not found", name);
                ctx.http
                    .create_message(channel)
                    .content(content)?
                    .await?
                    .reaction_delete(ctx, owner);
            }
            return Ok(None);
        }
        Err(why) => {
            if let Some(owner) = owner {
                ctx.http
                    .create_message(channel)
                    .content(OSU_API_ISSUE)?
                    .await?
                    .reaction_delete(ctx, owner);
            }
            return Err(why);
        }
    };
    let (globals_result, profile_result) = tokio::join!(
        super::get_globals_count(&ctx, &user.username, mode),
        ctx.clients
            .custom
            .get_osu_profile(user.user_id, mode, false)
    );
    let globals_count = match globals_result {
        Ok(globals_count) => globals_count,
        Err(why) => {
            error!("Error while requesting globals count: {}", why);
            BTreeMap::new()
        }
    };
    let profile = match profile_result {
        Ok((profile, _)) => profile,
        Err(why) => {
            if let Some(owner) = owner {
                ctx.http
                    .create_message(channel)
                    .content(OSU_WEB_ISSUE)?
                    .await?
                    .reaction_delete(ctx, owner);
            }
            return Err(why);
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

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());
    let retrieving_msg = if scores.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );
        ctx.http
            .create_message(channel)
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
    // Add missing maps to database
    if !missing_indices.is_empty() {
        let maps: Vec<_> = score_maps
            .par_iter()
            .enumerate()
            .filter(|(i, _)| missing_indices.contains(i))
            .map(|(_, (_, map))| map.clone())
            .collect();
        match ctx.psql().insert_beatmaps(&maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    };

    // Check if user has top scores on their own maps
    let own_top_scores =
        if profile.ranked_and_approved_beatmapset_count + profile.loved_beatmapset_count > 0 {
            score_maps
                .iter()
                .map(|(_, map)| map)
                .filter(|map| map.creator == user.username)
                .count()
        } else {
            0
        };

    // Calculate profile stats
    let profile_result = if score_maps.is_empty() {
        None
    } else {
        Some(ProfileResult::calc(mode, score_maps))
    };

    // Accumulate all necessary data
    let data = ProfileEmbed::new(
        user,
        profile_result,
        globals_count,
        &profile,
        own_top_scores,
    );

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }
    Ok(Some((data, profile)))
}

#[command]
#[short_desc("Display statistics of a user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("profile")]
pub async fn osu(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a mania user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("profilemania", "maniaprofile", "profilem")]
pub async fn mania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a taiko user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("profiletaiko", "taikoprofile", "profilet")]
pub async fn taiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    profile_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display statistics of a ctb user")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("profilectb", "ctbprofile", "profilec")]
pub async fn ctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
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

            let mut mapper = mappers.entry(map.creator).or_insert((0, 0.0));
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

const W: u32 = 1350;
const H: u32 = 350;

async fn graphs(profile: &OsuProfile) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    if profile.monthly_playcounts.len() < 2 {
        return Ok(None);
    }
    static LEN: usize = W as usize * H as usize;
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3
    {
        // Request all badge images
        let badges = match profile.badges.is_empty() {
            true => Vec::new(),
            false => {
                let badge_futs = profile.badges.iter().map(|badge| {
                    reqwest::get(&badge.image_url).and_then(|response| response.bytes())
                });
                try_join_all(badge_futs).await?
            }
        };

        // Setup total canvas
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        // Draw badges if there are any
        let canvas = if badges.is_empty() {
            root
        } else {
            let max_badges_per_row = 10;
            let margin = 5;
            let inner_margin = 3;
            let badge_count = badges.len() as u32;
            let badge_rows = ((badge_count - 1) / max_badges_per_row) + 1;
            let badge_total_height = (badge_rows * 60).min(H / 2);
            let badge_height = badge_total_height / badge_rows;
            let (top, bottom) = root.split_vertically(badge_total_height);
            let mut rows = Vec::with_capacity(badge_rows as usize);
            let mut last = top;
            for _ in 0..badge_rows {
                let (curr, remain) = last.split_vertically(badge_height);
                rows.push(curr);
                last = remain;
            }
            let badge_width =
                (W - 2 * margin - (max_badges_per_row - 1) * inner_margin) / max_badges_per_row;
            // Draw each row of badges
            for (row, chunk) in badges.chunks(max_badges_per_row as usize).enumerate() {
                let x_offset = (max_badges_per_row - chunk.len() as u32) * badge_width / 2;
                let mut chart_row = ChartBuilder::on(&rows[row])
                    .margin(margin)
                    .build_cartesian_2d(0..W, 0..badge_height)?;
                chart_row
                    .configure_mesh()
                    .disable_x_axis()
                    .disable_y_axis()
                    .disable_x_mesh()
                    .disable_y_mesh()
                    .draw()?;
                for (idx, badge) in chunk.iter().enumerate() {
                    let badge_img =
                        load_from_memory(badge)?.resize_exact(badge_width, badge_height, Lanczos3);
                    let x = x_offset + idx as u32 * badge_width + idx as u32 * inner_margin;
                    let y = badge_height;
                    let elem: BitMapElement<_> = ((x, y), badge_img).into();
                    chart_row.draw_series(std::iter::once(elem))?;
                }
            }
            bottom
        };

        let mut monthly_playcount = profile.monthly_playcounts.clone();
        let mut replays = profile.replays_watched_counts.clone();

        if replays.is_empty() {
            let iter = monthly_playcount
                .iter()
                .map(|DateCount { start_date, .. }| (*start_date, 0).into());
            replays = Vec::from_iter(iter);
        } else {
            let mut first = monthly_playcount.first().unwrap().start_date;
            if !replays.is_empty() {
                first = first.max(replays.first().unwrap().start_date);
            }

            let left_first: Vec<_> = monthly_playcount
                .iter()
                .take_while(|date_count| date_count.start_date < first)
                .map(|date_count| date_count.start_date)
                .collect();
            let right_first: Vec<_> = replays
                .iter()
                .take_while(|date_count| date_count.start_date < first)
                .map(|date_count| date_count.start_date)
                .collect();

            match left_first.len() > right_first.len() {
                true => spoof_date_count(&mut replays, left_first),
                false => spoof_date_count(&mut monthly_playcount, right_first),
            }
        }

        let left_first = monthly_playcount.first().unwrap().start_date;
        let left_last = monthly_playcount.last().unwrap().start_date;
        let left_max = monthly_playcount
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap();

        let right_first = replays.first().unwrap().start_date;
        let right_last = replays.last().unwrap().start_date;
        let right_max = replays
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap()
            .max(1);

        let right_label_area = match right_max {
            n if n < 10 => 40,
            n if n < 100 => 50,
            n if n < 1000 => 60,
            n if n < 10_000 => 70,
            n if n < 100_000 => 80,
            _ => 90,
        };

        let mut chart = ChartBuilder::on(&canvas)
            .margin(9)
            .x_label_area_size(20)
            .y_label_area_size(75)
            .right_y_label_area_size(right_label_area)
            .build_cartesian_2d((left_first..left_last).monthly(), 0..left_max)?
            .set_secondary_coord((right_first..right_last).monthly(), 0..right_max);

        // Mesh and labels
        chart
            .configure_mesh()
            .light_line_style(&BLACK.mix(0.0))
            // .disable_y_mesh()
            .disable_x_mesh()
            .x_labels(10)
            .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
            .y_desc("Monthly playcount")
            .label_style(("sans-serif", 20))
            .draw()?;
        chart
            .configure_secondary_axes()
            .y_desc("Replays watched")
            .label_style(("sans-serif", 20))
            .draw()?;

        // Draw playcount area
        chart
            .draw_series(
                AreaSeries::new(
                    monthly_playcount
                        .iter()
                        .map(|DateCount { start_date, count }| (*start_date, *count)),
                    0,
                    &BLUE.mix(0.2),
                )
                .border_style(&BLUE),
            )?
            .label("Monthly playcount")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(2)));

        // Draw circles
        chart.draw_series(
            monthly_playcount
                .iter()
                .map(|DateCount { start_date, count }| {
                    Circle::new((*start_date, *count), 2, BLUE.filled())
                }),
        )?;

        // Draw replay watched area
        chart
            .draw_secondary_series(
                AreaSeries::new(
                    replays
                        .iter()
                        .map(|DateCount { start_date, count }| (*start_date, *count)),
                    0,
                    &RED.mix(0.2),
                )
                .border_style(&RED),
            )?
            .label("Replays watched")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.stroke_width(2)));

        // Draw circles
        chart.draw_secondary_series(replays.iter().map(|DateCount { start_date, count }| {
            Circle::new((*start_date, *count), 2, RED.filled())
        }))?;

        // Legend
        chart
            .configure_series_labels()
            .background_style(&RGBColor(192, 192, 192))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(45)
            .label_font(("sans-serif", 20))
            .draw()?;
    }
    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;
    Ok(Some(png_bytes))
}

fn spoof_date_count(vec: &mut Vec<DateCount>, prefix: Vec<Date<Utc>>) {
    vec.reserve_exact(prefix.len());
    for date in prefix.into_iter().rev() {
        vec.insert(0, (date, 0).into());
    }
}
