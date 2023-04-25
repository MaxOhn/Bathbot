use std::{borrow::Cow, cmp::Reverse, collections::HashMap, fmt::Write, sync::Arc};

use bathbot_model::CountryCode;
use bathbot_psql::model::osu::DbScores;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    osu::ModSelection,
    CowUtils, IntHasher,
};
use eyre::{Report, Result};
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::{
    prelude::{GameMode, GameModsIntermode, OsuError},
    request::UserId,
};

use super::{ScoresOrder, ServerScores};
use crate::{
    commands::osu::{user_not_found, HasMods, ModsResult},
    core::Context,
    manager::redis::osu::UserArgs,
    pagination::ServerScoresPagination,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

pub async fn server_scores(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: ServerScores,
) -> Result<()> {
    let Some(guild_id) = command.guild_id else {
        let content = "This command does not work in DMs";
        command.error(&ctx, content).await?;

        return Ok(());
    };

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let mode = args.mode.map(GameMode::from);

    let country_code = match args.country {
        Some(ref country) => match CountryCode::from_name(country) {
            Some(code) => Some(Cow::Owned(code.to_string())),
            None if country.len() == 2 => Some(country.cow_to_ascii_uppercase()),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(&ctx, content).await?;

                return Ok(());
            }
        },
        None => None,
    };

    let guild_fut = ctx.cache.guild(guild_id);
    let members_fut = ctx.cache.members(guild_id);

    let (guild_res, members_res) = tokio::join!(guild_fut, members_fut);

    let guild_icon = guild_res
        .ok()
        .flatten()
        .and_then(|guild| Some((guild.id, *guild.icon.as_ref()?)));

    let members: Vec<_> = match members_res {
        Ok(members) => members.into_iter().map(|id| id as i64).collect(),
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let scores_fut =
        ctx.osu_scores()
            .from_discord_ids(&members, mode, mods.as_ref(), country_code.as_deref());

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let creator_id = match args.mapper {
        Some(ref mapper) => match UserArgs::username(&ctx, mapper).await {
            UserArgs::Args(args) => Some(args.user_id),
            UserArgs::User { user, .. } => Some(user.user_id),
            UserArgs::Err(OsuError::NotFound) => {
                let content = user_not_found(&ctx, UserId::Name(mapper.as_str().into())).await;
                command.error(&ctx, content).await?;

                return Ok(());
            }
            UserArgs::Err(err) => {
                let _ = command.error(&ctx, OSU_API_ISSUE).await;

                return Err(Report::new(err).wrap_err("Failed to get mapper"));
            }
        },
        None => None,
    };

    let content = msg_content(
        mods.as_ref(),
        args.mapper.as_deref(),
        country_code.as_deref(),
    );

    let sort = args.sort.unwrap_or_default();

    process_scores(&mut scores, creator_id, sort);

    ServerScoresPagination::builder(scores, mode, sort, guild_icon)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

fn process_scores(scores: &mut DbScores<IntHasher>, creator_id: Option<u32>, sort: ScoresOrder) {
    if let Some(creator_id) = creator_id {
        scores.retain(|score, maps, _, _| match maps.get(&score.map_id) {
            Some(map) => map.creator_id == creator_id,
            None => false,
        });
    }

    match sort {
        ScoresOrder::Acc => scores.scores_mut().sort_unstable_by(|a, b| {
            b.statistics
                .accuracy(b.mode)
                .total_cmp(&a.statistics.accuracy(a.mode))
        }),
        ScoresOrder::Ar => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let ars: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.ar))
                .collect();

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_ar = BeatmapAttributesBuilder::default()
                    .ar(ars[&a.map_id])
                    .mods(a.mods)
                    .build()
                    .ar;

                let b_ar = BeatmapAttributesBuilder::default()
                    .ar(ars[&b.map_id])
                    .mods(b.mods)
                    .build()
                    .ar;

                b_ar.total_cmp(&a_ar)
            })
        }
        ScoresOrder::Bpm => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let bpms: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.bpm))
                .collect();

            let mut clock_rates = HashMap::with_hasher(IntHasher);

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_clock_rate = *clock_rates
                    .entry(a.mods)
                    .or_insert_with(|| GameModsIntermode::from_bits(a.mods).legacy_clock_rate());

                let b_clock_rate = *clock_rates
                    .entry(b.mods)
                    .or_insert_with(|| GameModsIntermode::from_bits(b.mods).legacy_clock_rate());

                let a_bpm = bpms[&a.map_id] * a_clock_rate;
                let b_bpm = bpms[&b.map_id] * b_clock_rate;

                b_bpm.total_cmp(&a_bpm)
            })
        }
        ScoresOrder::Combo => scores
            .scores_mut()
            .sort_unstable_by_key(|score| Reverse(score.max_combo)),
        ScoresOrder::Cs => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let css: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.cs))
                .collect();

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_cs = BeatmapAttributesBuilder::default()
                    .cs(css[&a.map_id])
                    .mods(a.mods)
                    .build()
                    .cs;

                let b_cs = BeatmapAttributesBuilder::default()
                    .cs(css[&b.map_id])
                    .mods(b.mods)
                    .build()
                    .cs;

                b_cs.total_cmp(&a_cs)
            })
        }
        ScoresOrder::Date => scores
            .scores_mut()
            .sort_unstable_by_key(|score| Reverse(score.ended_at)),
        ScoresOrder::Hp => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let hps: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.hp))
                .collect();

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_ar = BeatmapAttributesBuilder::default()
                    .hp(hps[&a.map_id])
                    .mods(a.mods)
                    .build()
                    .hp;

                let b_hp = BeatmapAttributesBuilder::default()
                    .hp(hps[&b.map_id])
                    .mods(b.mods)
                    .build()
                    .hp;

                b_hp.total_cmp(&a_ar)
            })
        }
        ScoresOrder::Length => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let seconds_drain: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.seconds_drain))
                .collect();

            let mut clock_rates = HashMap::with_hasher(IntHasher);

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_clock_rate = *clock_rates
                    .entry(a.mods)
                    .or_insert_with(|| GameModsIntermode::from_bits(a.mods).legacy_clock_rate());

                let b_clock_rate = *clock_rates
                    .entry(b.mods)
                    .or_insert_with(|| GameModsIntermode::from_bits(b.mods).legacy_clock_rate());

                let a_drain = seconds_drain[&a.map_id] as f32 / a_clock_rate;
                let b_drain = seconds_drain[&b.map_id] as f32 / b_clock_rate;

                b_drain.total_cmp(&a_drain)
            })
        }
        ScoresOrder::Misses => scores
            .scores_mut()
            .sort_unstable_by_key(|score| Reverse(score.statistics.count_miss)),
        ScoresOrder::Od => {
            scores.retain(|score, maps, _, _| maps.get(&score.map_id).is_some());

            let ods: HashMap<_, _, IntHasher> = scores
                .maps()
                .map(|(map_id, map)| (*map_id, map.od))
                .collect();

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_od = BeatmapAttributesBuilder::default()
                    .od(ods[&a.map_id])
                    .mods(a.mods)
                    .build()
                    .od;

                let b_od = BeatmapAttributesBuilder::default()
                    .od(ods[&b.map_id])
                    .mods(b.mods)
                    .build()
                    .od;

                b_od.total_cmp(&a_od)
            })
        }
        ScoresOrder::Pp => {
            scores.retain(|score, _, _, _| score.pp.is_some());

            scores
                .scores_mut()
                .sort_unstable_by(|a, b| b.pp.unwrap().total_cmp(&a.pp.unwrap()))
        }
        ScoresOrder::RankedDate => {
            scores.retain(|score, maps, mapsets, _| {
                maps.get(&score.map_id)
                    .and_then(|map| mapsets.get(&map.mapset_id))
                    .and_then(|mapset| mapset.ranked_date)
                    .is_some()
            });

            let ranked_dates: HashMap<_, _, IntHasher> = scores
                .maps()
                .filter_map(|(map_id, map)| {
                    scores
                        .mapset(map.mapset_id)
                        .and_then(|mapset| Some((*map_id, mapset.ranked_date?)))
                })
                .collect();

            scores.scores_mut().sort_unstable_by(|a, b| {
                let a_ranked_date = ranked_dates[&a.map_id];
                let b_ranked_date = ranked_dates[&b.map_id];

                b_ranked_date.cmp(&a_ranked_date)
            });
        }
        ScoresOrder::Score => scores
            .scores_mut()
            .sort_unstable_by_key(|score| Reverse(score.score)),
        ScoresOrder::Stars => {
            scores.retain(|score, _, _, _| score.stars.is_some());

            scores
                .scores_mut()
                .sort_unstable_by(|a, b| b.stars.unwrap().total_cmp(&a.stars.unwrap()))
        }
    }
}

fn msg_content(mods: Option<&ModSelection>, mapper: Option<&str>, country: Option<&str>) -> String {
    let mut content = String::new();

    match mods {
        Some(ModSelection::Include(mods)) => {
            let _ = write!(content, "`Mods: Include {mods}`");
        }
        Some(ModSelection::Exclude(mods)) => {
            let _ = write!(content, "`Mods: Exclude {mods}`");
        }
        Some(ModSelection::Exact(mods)) => {
            let _ = write!(content, "`Mods: {mods}`");
        }
        None => {}
    }

    if let Some(mapper) = mapper {
        if !content.is_empty() {
            content.push_str(" • ");
        }

        let _ = write!(content, "`Mapper: {mapper}`");
    }

    if let Some(country) = country {
        if !content.is_empty() {
            content.push_str(" • ");
        }

        let _ = write!(content, "`Country: {country}`");
    }

    content
}
