use std::{cmp::Reverse, collections::HashMap, fmt::Write, sync::Arc};

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
    commands::GameModeOption,
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
    help = "List scores that the bot has stored.\n\
    The list will only contain scores that have been cached before i.e. \
    scores of the `/rs`, `/top`, `/pinned`, or `/cs` commands.\n\
    Similarly beatmaps or users won't be displayed if they're not cached.\n\
    To add a missing map, you can simply `<map [map url]` \
    and for missing users it's `<profile [username]`."
)]
/// List scores that the bot has stored
pub enum Scores {
    #[command(name = "server")]
    Server(ServerScores),
    #[command(name = "user")]
    User(UserScores),
    #[command(name = "map")]
    Map(MapScores),
}

#[derive(CreateCommand, CommandModel, HasMods)]
#[command(name = "server", dm_permission = false)]
/// List scores of members in this server
pub struct ServerScores {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<ScoresOrder>,
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<String>,
    /// Specify a country (code)
    country: Option<String>,
    /// Filter out scores on maps that don't match this status
    status: Option<MapStatus>,
    /// Only show scores on maps of that mapper
    mapper: Option<String>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    /// Specify a search query containing artist, stars, AR, BPM, ...
    query: Option<String>,
    /// Reverse the list
    reverse: Option<bool>,
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

#[derive(CreateCommand, CommandModel, HasMods, HasName)]
#[command(name = "user")]
/// List scores of a user
pub struct UserScores {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<ScoresOrder>,
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<String>,
    /// Filter out scores on maps that don't match this status
    status: Option<MapStatus>,
    /// Only show scores on maps of that mapper
    mapper: Option<String>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    /// Specify a search query containing artist, stars, AR, BPM, ...
    query: Option<String>,
    /// Reverse the list
    reverse: Option<bool>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CreateCommand, CommandModel, HasMods)]
#[command(name = "map")]
/// List scores on a map
pub struct MapScores {
    #[command(help = "Specify a map either by map url or map id.\n\
    If none is specified, it will search in the recent channel history \
    and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<String>,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<ScoresOrder>,
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<String>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, or limit values for \
        ar, cs, hp, od, bpm, length, stars, pp, combo, score, misses, date, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane date<2020-12-31 misses=1`"
    )]
    /// Specify a search query containing stars, AR, BPM, ...
    query: Option<String>,
    #[command(min_value = 1, max_value = 50)]
    /// While checking the channel history, I will choose the index-th map I can
    /// find
    index: Option<u32>,
    /// Reverse the list
    reverse: Option<bool>,
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
    reverse: Option<bool>,
) {
    if let Some(criteria) = criteria {
        let inner = criteria.inner();

        scores.retain(|score, maps, mapsets, _| {
            let mut matches = true;

            matches &= inner.combo.contains(score.max_combo);
            matches &= inner.miss.contains(score.statistics.count_miss);
            matches &= inner.score.contains(score.score);
            matches &= inner.date.contains(score.ended_at.date());

            if !inner.stars.is_empty() {
                let Some(stars) = score.stars else { return false };
                matches &= inner.stars.contains(stars);
            }

            if !inner.pp.is_empty() {
                let Some(pp) = score.pp else { return false };
                matches &= inner.pp.contains(pp);
            }

            if inner.ar.is_empty()
                && inner.cs.is_empty()
                && inner.hp.is_empty()
                && inner.od.is_empty()
                && inner.length.is_empty()
                && inner.bpm.is_empty()
                && inner.version.is_empty()
                && inner.artist.is_empty()
                && inner.title.is_empty()
                && inner.ranked_date.is_empty()
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

            matches &= inner.ar.contains(attrs.ar as f32);
            matches &= inner.cs.contains(attrs.cs as f32);
            matches &= inner.hp.contains(attrs.hp as f32);
            matches &= inner.od.contains(attrs.od as f32);

            let clock_rate = attrs.clock_rate as f32;
            inner.length.contains(map.seconds_drain as f32 / clock_rate);
            inner.bpm.contains(map.bpm * clock_rate);

            let version = map.version.cow_to_ascii_lowercase();
            matches &= inner.version.matches(&version);

            if inner.artist.is_empty()
                && inner.title.is_empty()
                && inner.ranked_date.is_empty()
                && !criteria.has_search_terms()
            {
                return matches;
            }

            let Some(mapset) = mapsets.get(&map.mapset_id) else { return false };

            if !inner.ranked_date.is_empty() {
                let Some(datetime) = mapset.ranked_date else { return false };
                matches &= inner.ranked_date.contains(datetime.date());
            }

            let artist = mapset.artist.cow_to_ascii_lowercase();
            matches &= inner.artist.matches(&artist);

            let title = mapset.title.cow_to_ascii_lowercase();
            matches &= inner.title.matches(&title);

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

    if reverse == Some(true) {
        scores.scores_mut().reverse();
    }
}

fn separate_content(content: &mut String) {
    if !content.is_empty() {
        content.push_str(" • ");
    }
}

fn criteria_to_content(content: &mut String, criteria: &FilterCriteria<ScoresCriteria<'_>>) {
    let ScoresCriteria {
        ar,
        cs,
        hp,
        od,
        length,
        stars,
        pp,
        bpm,
        combo,
        miss,
        score,
        date,
        ranked_date,
        artist,
        title,
        version,
    } = criteria.inner();

    let mut only_search_text = true;

    if !ar.is_empty() {
        separate_content(content);
        let _ = write!(content, "`AR: {ar:?}`");
        only_search_text = false;
    }

    if !cs.is_empty() {
        separate_content(content);
        let _ = write!(content, "`CS: {cs:?}`");
        only_search_text = false;
    }

    if !hp.is_empty() {
        separate_content(content);
        let _ = write!(content, "`HP: {hp:?}`");
        only_search_text = false;
    }

    if !od.is_empty() {
        separate_content(content);
        let _ = write!(content, "`OD: {od:?}`");
        only_search_text = false;
    }

    if !length.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Length: {length:?}`");
        only_search_text = false;
    }

    if !stars.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Stars: {stars:?}`");
        only_search_text = false;
    }

    if !bpm.is_empty() {
        separate_content(content);
        let _ = write!(content, "`BPM: {bpm:?}`");
        only_search_text = false;
    }

    if !combo.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Combo: {combo:?}`");
        only_search_text = false;
    }

    if !miss.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Misses: {miss:?}`");
        only_search_text = false;
    }

    if !score.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Score: {score:?}`");
        only_search_text = false;
    }

    if !pp.is_empty() {
        separate_content(content);
        let _ = write!(content, "`PP: {pp:?}`");
        only_search_text = false;
    }

    if !artist.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Artist: {artist:?}`");
        only_search_text = false;
    }

    if !title.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Title: {title:?}`");
        only_search_text = false;
    }

    if !version.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Version: {version:?}`");
        only_search_text = false;
    }

    if !date.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Date: {date:?}`");
        only_search_text = false;
    }

    if !ranked_date.is_empty() {
        separate_content(content);
        let _ = write!(content, "`Ranked: {ranked_date:?}`");
        only_search_text = false;
    }

    if criteria.has_search_terms() {
        separate_content(content);

        if only_search_text {
            content.push_str("`Query: ");
        } else {
            content.push_str("`Remaining query: ");
        }

        content.push_str(criteria.search_text());
        content.push('`');
    }
}
