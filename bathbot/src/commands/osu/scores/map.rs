use std::{borrow::Cow, cmp::Reverse, collections::HashMap, fmt::Write, sync::Arc};

use bathbot_psql::model::osu::DbScores;
use bathbot_util::{
    constants::GENERAL_ISSUE,
    matcher,
    osu::{MapIdType, ModSelection},
    IntHasher, MessageBuilder,
};
use eyre::Result;
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use twilight_interactions::command::AutocompleteValue;

use super::{MapScores, ScoresOrder};
use crate::{
    commands::osu::{
        compare::{slash_compare_score, ScoreOrder},
        CompareScoreAutocomplete, HasMods, ModsResult,
    },
    core::Context,
    pagination::MapScoresPagination,
    util::{interaction::InteractionCommand, Authored, CheckPermissions, InteractionCommandExt},
};

pub async fn map_scores(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: MapScores,
) -> Result<()> {
    let Some(guild_id) = command.guild_id else {
        // TODO: use mode when /cs uses it
        let MapScores { map, mode: _, sort, mods, index } = args;

        let sort = match sort {
            Some(ScoresOrder::Acc) => Some(ScoreOrder::Acc),
            Some(ScoresOrder::Combo) => Some(ScoreOrder::Combo),
            Some(ScoresOrder::Date) => Some(ScoreOrder::Date),
            Some(ScoresOrder::Misses) => Some(ScoreOrder::Misses),
            Some(ScoresOrder::Pp) => Some(ScoreOrder::Pp),
            Some(ScoresOrder::Score) => Some(ScoreOrder::Score),
            Some(ScoresOrder::Stars) => Some(ScoreOrder::Stars),
            None => None,
            Some(ScoresOrder::Ar |
            ScoresOrder::Bpm |
            ScoresOrder::Cs |
            ScoresOrder::Hp |
            ScoresOrder::Length |
            ScoresOrder::Od |
            ScoresOrder::RankedDate) => {
                let content = "When using this command in DMs, \
                the only available sort orders are \
                `Accuracy`, `Combo`, `Date`, `Misses`, `PP`, `Score`, or `Stars`";
                command.error(&ctx, content).await?;

                return Ok(());
            }
        };

        let args = CompareScoreAutocomplete {
            name: None,
            map: map.map(Cow::Owned),
            difficulty: AutocompleteValue::None,
            sort,
            mods: mods.map(Cow::Owned),
            index,
            discord: None,
        };

        return slash_compare_score(ctx, &mut command, args).await;
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

    let map = match args.map {
        Some(ref map) => {
            let map_opt = matcher::get_osu_map_id(map)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(map).map(MapIdType::Set));

            if map_opt.is_none() {
                let content =
                    "Failed to parse map url. Be sure you specify a valid map id or url to a map.";
                command.error(&ctx, content).await?;

                return Ok(());
            }

            map_opt
        }
        None => None,
    };

    let map_id = match map {
        Some(MapIdType::Map(id)) => id,
        Some(MapIdType::Set(_)) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        None if command.can_read_history() => {
            let msgs = match ctx.retrieve_channel_history(command.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to retrieve channel history"));
                }
            };

            match MapIdType::map_from_msgs(&msgs, 0) {
                Some(id) => id,
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";
                    command.error(&ctx, content).await?;

                    return Ok(());
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
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
            .from_discord_ids(&members, mode, mods.as_ref(), None, Some(map_id));

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if scores.is_empty() {
        let content = format!(
            "Looks like I don't have any stored{mode} scores on map id {map_id}{mods}",
            mode = match mode {
                Some(GameMode::Osu) => " osu!",
                Some(GameMode::Taiko) => " taiko",
                Some(GameMode::Catch) => " catch",
                Some(GameMode::Mania) => " mania",
                None => "",
            },
            mods = match mods {
                Some(_) => " for the specified mods",
                None => "",
            }
        );
        let builder = MessageBuilder::new().embed(content);
        command.update(&ctx, &builder).await?;

        return Ok(());
    } else if scores.maps().next().zip(scores.mapsets().next()).is_none() {
        let content = format!("Looks like I don't have map id {map_id} stored");
        let builder = MessageBuilder::new().embed(content);
        command.update(&ctx, &builder).await?;

        return Ok(());
    }

    let sort = args.sort.unwrap_or_default();
    let content = msg_content(sort, mods.as_ref());

    process_scores(&mut scores, sort);

    MapScoresPagination::builder(scores, mode, sort, guild_icon)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

fn process_scores(scores: &mut DbScores<IntHasher>, sort: ScoresOrder) {
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

fn msg_content(sort: ScoresOrder, mods: Option<&ModSelection>) -> String {
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

    if !content.is_empty() {
        content.push_str(" • ");
    }

    content.push_str("`Order: ");

    let order = match sort {
        ScoresOrder::Acc => "Accuracy",
        ScoresOrder::Ar => "AR",
        ScoresOrder::Bpm => "BPM",
        ScoresOrder::Combo => "Combo",
        ScoresOrder::Cs => "CS",
        ScoresOrder::Date => "Date",
        ScoresOrder::Hp => "HP",
        ScoresOrder::Length => "Length",
        ScoresOrder::Misses => "Miss count",
        ScoresOrder::Od => "OD",
        ScoresOrder::Pp => "PP",
        ScoresOrder::RankedDate => "Ranked date",
        ScoresOrder::Score => "Score",
        ScoresOrder::Stars => "Stars",
    };

    content.push_str(order);
    content.push('`');

    content
}
