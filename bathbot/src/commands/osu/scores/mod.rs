use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_psql::model::osu::DbScores;
use bathbot_util::{CowUtils, IntHasher};
use eyre::Result;
use rosu_pp::{beatmap::BeatmapAttributesBuilder, GameMode as GameModePp};
use rosu_v2::prelude::{GameMode, GameModsIntermode, RankStatus};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use self::{map::map_scores, server::server_scores, user::user_scores};
use crate::{
    commands::GradeOption,
    core::Context,
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, ScoresCriteria},
        InteractionCommandExt,
    },
};

mod map;
mod server;
mod user;

#[derive(CreateCommand, CommandModel, SlashCommand)]
#[command(
    name = "scores",
    desc = "List scores that the bot has stored",
    help = "List scores that the bot has stored.\n\
    The list will only contain scores that have been cached before i.e. \
    scores of the `/rs`, `/top`, `/pinned`, or `/cs` commands.\n\
    Similarly beatmaps or users won't be displayed if they're not cached.\n\
    To add a missing map, you can simply `<map [map url]` \
    and for missing users it's `<profile [username]`."
)]
pub enum Scores {
    #[command(name = "server")]
    Server(ServerScores),
    #[command(name = "user")]
    User(UserScores),
    #[command(name = "map")]
    Map(MapScores),
}

#[derive(CreateCommand, CommandModel, HasMods)]
#[command(
    name = "server",
    dm_permission = false,
    desc = "List scores of members in this server"
)]
pub struct ServerScores {
    #[command(desc = "Specify a gamemode")]
    mode: Option<ScoresGameMode>,
    #[command(desc = "Choose how the scores should be ordered, defaults to PP")]
    sort: Option<ScoresOrder>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)"
    )]
    mods: Option<String>,
    #[command(desc = "Specify a country (code)")]
    country: Option<String>,
    #[command(desc = "Filter out scores on maps that don't match this status")]
    status: Option<MapStatus>,
    #[command(desc = "Only show scores on maps of that mapper")]
    mapper: Option<String>,
    #[command(
        desc = "Specify a search query containing artist, stars, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    query: Option<String>,
    #[command(desc = "Only include each user's best score or all scores")]
    per_user: Option<ScoresPerUser>,
    #[command(desc = "Reverse the list")]
    reverse: Option<bool>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum ScoresGameMode {
    #[option(name = "all", value = "all")]
    All,
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
    #[option(name = "mania", value = "mania")]
    Mania,
}

impl From<ScoresGameMode> for Option<GameMode> {
    fn from(mode: ScoresGameMode) -> Self {
        match mode {
            ScoresGameMode::All => None,
            ScoresGameMode::Osu => Some(GameMode::Osu),
            ScoresGameMode::Taiko => Some(GameMode::Taiko),
            ScoresGameMode::Catch => Some(GameMode::Catch),
            ScoresGameMode::Mania => Some(GameMode::Mania),
        }
    }
}
#[derive(Copy, Clone, CommandOption, CreateOption, Default)]
pub enum ScoresOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "AR", value = "ar")]
    Ar,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "CS", value = "cs")]
    Cs,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "HP", value = "hp")]
    Hp,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "OD", value = "od")]
    Od,
    #[option(name = "PP", value = "pp")]
    #[default]
    Pp,
    #[option(name = "Ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
enum MapStatus {
    #[option(name = "Ranked", value = "ranked")]
    Ranked,
    #[option(name = "Loved", value = "loved")]
    Loved,
    #[option(name = "Approved", value = "approved")]
    Approved,
}

impl From<MapStatus> for RankStatus {
    fn from(status: MapStatus) -> Self {
        match status {
            MapStatus::Ranked => RankStatus::Ranked,
            MapStatus::Loved => RankStatus::Loved,
            MapStatus::Approved => RankStatus::Approved,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
enum ScoresPerUser {
    #[option(name = "All", value = "all")]
    All,
    #[option(name = "Best", value = "best")]
    Best,
}

#[derive(CreateCommand, CommandModel, HasMods, HasName)]
#[command(name = "user", desc = "List scores of a user")]
pub struct UserScores {
    #[command(desc = "Specify a gamemode")]
    mode: Option<ScoresGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "Choose how the scores should be ordered, defaults to PP")]
    sort: Option<ScoresOrder>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)"
    )]
    mods: Option<String>,
    #[command(desc = "Filter out scores on maps that don't match this status")]
    status: Option<MapStatus>,
    #[command(desc = "Only show scores on maps of that mapper")]
    mapper: Option<String>,
    #[command(
        desc = "Specify a search query containing artist, stars, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    query: Option<String>,
    #[command(desc = "Reverse the list")]
    reverse: Option<bool>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CreateCommand, CommandModel, HasMods)]
#[command(name = "map", desc = "List scores on a map")]
pub struct MapScores {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<String>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<ScoresGameMode>,
    #[command(desc = "Choose how the scores should be ordered, defaults to PP")]
    sort: Option<ScoresOrder>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)"
    )]
    mods: Option<String>,
    #[command(desc = "Specify a country (code)")]
    country: Option<String>,
    #[command(
        desc = "Specify a search query containing stars, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    query: Option<String>,
    #[command(desc = "Only include each user's best score or all scores")]
    per_user: Option<ScoresPerUser>,
    #[command(
        min_value = 1,
        max_value = 50,
        desc = "While checking the channel history, I will choose the index-th map I can find"
    )]
    index: Option<u32>,
    #[command(desc = "Reverse the list")]
    reverse: Option<bool>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
}

async fn slash_scores(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Scores::from_interaction(command.input_data())? {
        Scores::Server(args) => server_scores(ctx, command, args).await,
        Scores::User(args) => user_scores(ctx, command, args).await,
        Scores::Map(args) => map_scores(ctx, command, args).await,
    }
}

fn process_scores(
    scores: &mut DbScores<IntHasher>,
    creator_id: Option<u32>,
    sort: ScoresOrder,
    status: Option<MapStatus>,
    criteria: Option<&FilterCriteria<ScoresCriteria<'_>>>,
    per_user: Option<ScoresPerUser>,
    reverse: Option<bool>,
) {
    if let Some(criteria) = criteria {
        scores.retain(|score, maps, mapsets, _| {
            let mut matches = true;

            matches &= criteria.combo.contains(score.max_combo);
            matches &= criteria.miss.contains(score.statistics.count_miss);
            matches &= criteria.score.contains(score.score);
            matches &= criteria.date.contains(score.ended_at.date());

            if !criteria.stars.is_empty() {
                let Some(stars) = score.stars else { return false };
                matches &= criteria.stars.contains(stars);
            }

            if !criteria.pp.is_empty() {
                let Some(pp) = score.pp else { return false };
                matches &= criteria.pp.contains(pp);
            }

            if criteria.ar.is_empty()
                && criteria.cs.is_empty()
                && criteria.hp.is_empty()
                && criteria.od.is_empty()
                && criteria.length.is_empty()
                && criteria.bpm.is_empty()
                && criteria.version.is_empty()
                && criteria.artist.is_empty()
                && criteria.title.is_empty()
                && criteria.ranked_date.is_empty()
                && !criteria.has_search_terms()
            {
                return matches;
            }

            let Some(map) = maps.get(&score.map_id) else { return false };

            let attrs = BeatmapAttributesBuilder::default()
                .ar(map.ar)
                .cs(map.cs)
                .hp(map.hp)
                .od(map.od)
                .mods(score.mods)
                .mode(match score.mode {
                    GameMode::Osu => GameModePp::Osu,
                    GameMode::Taiko => GameModePp::Taiko,
                    GameMode::Catch => GameModePp::Catch,
                    GameMode::Mania => GameModePp::Mania,
                })
                // TODO: maybe add gamemode to DbBeatmap so we can check for converts
                .build();

            matches &= criteria.ar.contains(attrs.ar as f32);
            matches &= criteria.cs.contains(attrs.cs as f32);
            matches &= criteria.hp.contains(attrs.hp as f32);
            matches &= criteria.od.contains(attrs.od as f32);

            let clock_rate = attrs.clock_rate as f32;
            matches &= criteria
                .length
                .contains(map.seconds_drain as f32 / clock_rate);
            matches &= criteria.bpm.contains(map.bpm * clock_rate);

            let version = map.version.cow_to_ascii_lowercase();
            matches &= criteria.version.matches(&version);

            if criteria.artist.is_empty()
                && criteria.title.is_empty()
                && criteria.ranked_date.is_empty()
                && !criteria.has_search_terms()
            {
                return matches;
            }

            let Some(mapset) = mapsets.get(&map.mapset_id) else { return false };

            if !criteria.ranked_date.is_empty() {
                let Some(datetime) = mapset.ranked_date else { return false };
                matches &= criteria.ranked_date.contains(datetime.date());
            }

            let artist = mapset.artist.cow_to_ascii_lowercase();
            matches &= criteria.artist.matches(&artist);

            let title = mapset.title.cow_to_ascii_lowercase();
            matches &= criteria.title.matches(&title);

            if matches && criteria.has_search_terms() {
                let terms = [artist, title, version];

                matches &= criteria
                    .search_terms()
                    .all(|term| terms.iter().any(|searchable| searchable.contains(term)))
            }

            matches
        });
    }

    if let Some(creator_id) = creator_id {
        scores.retain(|score, maps, _, _| match maps.get(&score.map_id) {
            Some(map) => map.creator_id == creator_id,
            None => false,
        });
    }

    if let Some(status) = status.map(RankStatus::from) {
        scores.retain(|score, maps, mapsets, _| {
            let Some(map) = maps.get(&score.map_id) else { return false };
            let Some(mapset) = mapsets.get(&map.mapset_id) else { return false };

            mapset.rank_status == status
        })
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

            scores.scores_mut().sort_unstable_by(|a, b| {
                b.pp.unwrap()
                    .total_cmp(&a.pp.unwrap())
                    .then_with(|| a.score_id.cmp(&b.score_id))
            });
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
            .sort_unstable_by_key(|score| (Reverse(score.score), score.score_id)),
        ScoresOrder::Stars => {
            scores.retain(|score, _, _, _| score.stars.is_some());

            scores
                .scores_mut()
                .sort_unstable_by(|a, b| b.stars.unwrap().total_cmp(&a.stars.unwrap()))
        }
    }

    if reverse == Some(true) {
        scores.scores_mut().reverse();
    }

    match per_user {
        Some(ScoresPerUser::All) | None => {}
        Some(ScoresPerUser::Best) => {
            let mut seen = HashSet::with_capacity_and_hasher(scores.user_count(), IntHasher);

            scores.retain(|score, _, _, _| seen.insert(score.user_id));
        }
    }
}

fn separate_content(content: &mut String) {
    if !content.is_empty() {
        content.push_str(" • ");
    }
}

async fn get_mode(
    ctx: &Context,
    mode: Option<ScoresGameMode>,
    user_id: Id<UserMarker>,
) -> Result<Option<GameMode>> {
    if let Some(mode) = mode {
        return Ok(mode.into());
    }

    ctx.user_config().mode(user_id).await
}
