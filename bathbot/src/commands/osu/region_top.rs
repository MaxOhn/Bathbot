use std::{borrow::Cow, cmp::Reverse, collections::HashMap, convert::identity, fmt::Write, mem};

use bathbot_macros::{HasMods, SlashCommand};
use bathbot_model::{command_fields::GameModeOption, Countries};
use bathbot_psql::model::osu::{DbScoreBeatmap, DbScoreBeatmapset, DbTopScore, DbTopScores};
use bathbot_util::{constants::GENERAL_ISSUE, osu::ModSelection, CowUtils, IntHasher};
use compact_str::CompactString;
use eyre::Result;
use rkyv::collections::ArchivedHashMap;
use rosu_pp::model::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use twilight_interactions::command::{AutocompleteValue, CommandModel, CreateCommand};
use twilight_model::application::command::{CommandOptionChoice, CommandOptionChoiceValue};

use crate::{
    active::{impls::RegionTopPagination, ActiveMessages},
    commands::osu::{HasMods, ModsResult, ScoresOrder},
    core::Context,
    manager::redis::RedisData,
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, IFilterCriteria, ScoresCriteria, Searchable},
        Authored, InteractionCommandExt,
    },
};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "regiontop",
    desc = "Display top scores of a region",
    help = "Display top scores of a region.\n\
    If no country is specified, it will show the global top100.\n\
    If a country is specified but not a region, it will show that country's top100.\n\
    If a country and a region is specified, it will show that region's top100. \
    Region-based user data is provided by <https://osuworld.octo.moe> so if you think your scores are missing, \
    be sure you select a region for yourself on the website.\n\n\
    All score data originates from the bathbot database and is **updated once per hour**. \
    If the leaderboard contains restricted players, \
    you can `<osu` them so that they'll get filtered out on the next update. \
    If you think a player is missing, you can use `<top` on them so that they'll appear on the \
    next update (if they enlisted themselves for a region on the website)."
)]
#[allow(unused)] // only used to create the command
pub struct RegionTop {
    #[command(desc = "Specify a country (code), defaults to global")]
    pub country: Option<String>,
    #[command(autocomplete = true, desc = "Specify a region within the country")]
    pub region: Option<String>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Choose how the scores should be ordered, defaults to PP")]
    sort: Option<ScoresOrder>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<String>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(desc = "Search for a specific artist, title, difficulty, or mapper")]
    query: Option<String>,
}

#[derive(CommandModel, HasMods)]
#[command(autocomplete = true)]
struct RegionTopInput {
    country: Option<String>,
    region: AutocompleteValue<String>,
    mode: Option<GameModeOption>,
    sort: Option<ScoresOrder>,
    mods: Option<String>,
    reverse: Option<bool>,
    query: Option<String>,
}

pub async fn slash_regiontop(mut command: InteractionCommand) -> Result<()> {
    let input = RegionTopInput::from_interaction(command.input_data())?;
    let mods = input.mods();

    let RegionTopInput {
        country,
        mut region,
        mode,
        sort,
        mods: _,
        reverse,
        query,
    } = input;

    let region_opt = match region {
        AutocompleteValue::None => None,
        AutocompleteValue::Focused(ref region) => {
            return handle_autocomplete(&command, country.as_deref(), region).await
        }
        AutocompleteValue::Completed(ref mut region) => Some(mem::take(region)),
    };

    let mods = match mods {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(content).await?;

            return Ok(());
        }
    };

    let country_code = country
        .as_deref()
        .and_then(|country| {
            Countries::name(country)
                .to_code()
                .map(Cow::Borrowed)
                .or_else(|| (country.len() == 2).then_some(Countries::code(country).uppercase()))
        })
        .map(|code| CompactString::from(code.as_ref()));

    let region = match (country_code.as_deref(), region_opt.as_deref()) {
        (Some(country_code), Some(region)) => match Context::redis().country_regions().await? {
            RedisData::Original(ref country_regions) => country_regions
                .get(country_code)
                .and_then(|regions| regions.iter().find(|(_, name)| region == name.as_str()))
                .map(|(code, name)| Region {
                    code: code.to_owned(),
                    name: name.to_owned(),
                }),
            RedisData::Archive(ref country_regions) => country_regions
                .get(country_code)
                .and_then(|regions| regions.iter().find(|(_, name)| region == name.as_str()))
                .map(|(code, name)| Region {
                    code: code.as_str().into(),
                    name: name.as_str().into(),
                }),
        },
        _ => None,
    };

    let mode = match mode.map(GameMode::from) {
        Some(mode) => mode,
        None => match Context::user_config().mode(command.user_id()?).await {
            Ok(Some(mode)) => mode,
            Ok(None) => GameMode::Osu,
            Err(err) => {
                warn!(?err);

                GameMode::Osu
            }
        },
    };

    let args = RegionTopArgs {
        mode,
        sort: sort.unwrap_or_default(),
        mods,
        reverse,
        query,
    };

    match (country_code, region) {
        (Some(_), None) if region_opt.is_some() => {
            let content = "Invalid region for the country";
            command.error(content).await?;

            Ok(())
        }
        (None, _) => regiontop_global(&mut command, args).await,
        (Some(country), None) => regiontop_country(&mut command, country, args).await,
        (Some(country), Some(region)) => {
            regiontop_region(&mut command, country, region, args).await
        }
    }
}

async fn regiontop_global(command: &mut InteractionCommand, args: RegionTopArgs) -> Result<()> {
    let scores = match Context::osu_scores()
        .db_top_scores(args.mode, None, None)
        .await
    {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let kind = RegionTopKind::Global;

    run_pagination(command, kind, scores, &args).await
}

async fn regiontop_country(
    command: &mut InteractionCommand,
    country_code: CompactString,
    args: RegionTopArgs,
) -> Result<()> {
    let code = Some(country_code.as_str());

    let scores = match Context::osu_scores()
        .db_top_scores(args.mode, None, code)
        .await
    {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let kind = RegionTopKind::Country { country_code };

    run_pagination(command, kind, scores, &args).await
}

async fn regiontop_region(
    command: &mut InteractionCommand,
    country_code: CompactString,
    region: Region,
    args: RegionTopArgs,
) -> Result<()> {
    let user_ids = match Context::client().get_region_user_ids(&region.code).await {
        Ok(user_ids) => user_ids,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let code = Some(country_code.as_str());
    let scores_fut = Context::osu_scores().db_top_scores(args.mode, Some(&user_ids), code);

    let scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let kind = RegionTopKind::Region {
        country_code,
        region_name: region.name,
    };

    run_pagination(command, kind, scores, &args).await
}

async fn run_pagination(
    command: &mut InteractionCommand,
    kind: RegionTopKind,
    mut scores: DbTopScores<IntHasher>,
    args: &RegionTopArgs,
) -> Result<()> {
    args.process_scores(&mut scores);

    let owner = command.user_id()?;

    let pagination = RegionTopPagination::builder()
        .kind(kind)
        .scores(scores)
        .mode(args.mode)
        .sort(args.sort)
        .msg_owner(owner)
        .content(args.content())
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(command)
        .await
}

struct Region {
    name: CompactString,
    code: CompactString,
}

struct RegionTopArgs {
    mode: GameMode,
    sort: ScoresOrder,
    mods: Option<ModSelection>,
    reverse: Option<bool>,
    query: Option<String>,
}

impl RegionTopArgs {
    fn process_scores(&self, scores: &mut DbTopScores<IntHasher>) {
        let mode = self.mode;

        if let Some(ref query) = self.query {
            let criteria = ScoresCriteria::create(query);
            scores.retain(|score, maps, mapsets| (mode, score, maps, mapsets).matches(&criteria));
        }

        if let Some(ref mods) = self.mods {
            match mods {
                ModSelection::Include(mods) | ModSelection::Exact(mods) if mods.is_empty() => {
                    scores.retain(|score, _, _| score.mods == 0)
                }
                ModSelection::Include(mods) => {
                    let bits = mods.bits();
                    scores.retain(|score, _, _| score.mods & bits == bits);
                }
                ModSelection::Exclude(mods) if mods.is_empty() => {
                    scores.retain(|score, _, _| score.mods > 0)
                }
                ModSelection::Exclude(mods) => {
                    let bits = mods.bits();
                    scores.retain(|score, _, _| score.mods & bits == 0);
                }
                ModSelection::Exact(mods) => {
                    let bits = mods.bits();
                    scores.retain(|score, _, _| score.mods == bits);
                }
            }
        }

        match self.sort {
            ScoresOrder::Acc => scores.scores_mut().sort_unstable_by(|a, b| {
                b.statistics
                    .accuracy(mode)
                    .total_cmp(&a.statistics.accuracy(mode))
            }),
            ScoresOrder::Ar => {
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let ars: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.ar))
                    .collect();

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_ar = BeatmapAttributesBuilder::default()
                        .ar(ars[&a.map_id], false)
                        .mods(a.mods)
                        .build()
                        .ar;

                    let b_ar = BeatmapAttributesBuilder::default()
                        .ar(ars[&b.map_id], false)
                        .mods(b.mods)
                        .build()
                        .ar;

                    b_ar.total_cmp(&a_ar)
                })
            }
            ScoresOrder::Bpm => {
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let bpms: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.bpm))
                    .collect();

                let mut clock_rates = HashMap::with_hasher(IntHasher);

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_clock_rate = *clock_rates.entry(a.mods).or_insert_with(|| {
                        GameModsIntermode::from_bits(a.mods).legacy_clock_rate()
                    });

                    let b_clock_rate = *clock_rates.entry(b.mods).or_insert_with(|| {
                        GameModsIntermode::from_bits(b.mods).legacy_clock_rate()
                    });

                    let a_bpm = bpms[&a.map_id] * a_clock_rate;
                    let b_bpm = bpms[&b.map_id] * b_clock_rate;

                    b_bpm.total_cmp(&a_bpm)
                })
            }
            ScoresOrder::Combo => scores
                .scores_mut()
                .sort_unstable_by_key(|score| Reverse(score.max_combo)),
            ScoresOrder::Cs => {
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let css: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.cs))
                    .collect();

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_cs = BeatmapAttributesBuilder::default()
                        .cs(css[&a.map_id], false)
                        .mods(a.mods)
                        .build()
                        .cs;

                    let b_cs = BeatmapAttributesBuilder::default()
                        .cs(css[&b.map_id], false)
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
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let hps: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.hp))
                    .collect();

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_ar = BeatmapAttributesBuilder::default()
                        .hp(hps[&a.map_id], false)
                        .mods(a.mods)
                        .build()
                        .hp;

                    let b_hp = BeatmapAttributesBuilder::default()
                        .hp(hps[&b.map_id], false)
                        .mods(b.mods)
                        .build()
                        .hp;

                    b_hp.total_cmp(&a_ar)
                })
            }
            ScoresOrder::Length => {
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let seconds_drain: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.seconds_drain))
                    .collect();

                let mut clock_rates = HashMap::with_hasher(IntHasher);

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_clock_rate = *clock_rates.entry(a.mods).or_insert_with(|| {
                        GameModsIntermode::from_bits(a.mods).legacy_clock_rate()
                    });

                    let b_clock_rate = *clock_rates.entry(b.mods).or_insert_with(|| {
                        GameModsIntermode::from_bits(b.mods).legacy_clock_rate()
                    });

                    let a_drain = seconds_drain[&a.map_id] as f32 / a_clock_rate;
                    let b_drain = seconds_drain[&b.map_id] as f32 / b_clock_rate;

                    b_drain.total_cmp(&a_drain)
                })
            }
            ScoresOrder::Misses => scores
                .scores_mut()
                .sort_unstable_by_key(|score| Reverse(score.statistics.miss)),
            ScoresOrder::Od => {
                scores.retain(|score, maps, _| maps.get(&score.map_id).is_some());

                let ods: HashMap<_, _, IntHasher> = scores
                    .maps()
                    .map(|(map_id, map)| (*map_id, map.od))
                    .collect();

                scores.scores_mut().sort_unstable_by(|a, b| {
                    let a_od = BeatmapAttributesBuilder::default()
                        .od(ods[&a.map_id], false)
                        .mods(a.mods)
                        .build()
                        .od;

                    let b_od = BeatmapAttributesBuilder::default()
                        .od(ods[&b.map_id], false)
                        .mods(b.mods)
                        .build()
                        .od;

                    b_od.total_cmp(&a_od)
                })
            }
            ScoresOrder::Pp => {
                scores.scores_mut().sort_unstable_by(|a, b| {
                    b.pp.total_cmp(&a.pp)
                        .then_with(|| a.score_id.cmp(&b.score_id))
                });
            }
            ScoresOrder::RankedDate => {
                scores.retain(|score, maps, mapsets| {
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
                scores.retain(|score, _, _| score.stars.is_some());

                scores
                    .scores_mut()
                    .sort_unstable_by(|a, b| b.stars.unwrap().total_cmp(&a.stars.unwrap()))
            }
        }

        if self.reverse.is_some_and(identity) {
            scores.scores_mut().reverse();
        }
    }

    fn content(&self) -> Box<str> {
        fn separate_content(content: &mut String) {
            if !content.is_empty() {
                content.push_str(" • ");
            }
        }

        let mut content = String::new();

        match self.mods.as_ref() {
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

        if let Some(ref query) = self.query {
            let criteria = ScoresCriteria::create(query);
            criteria.display(&mut content);
        }

        separate_content(&mut content);

        content.push_str("`Order: ");

        let order = match self.sort {
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

        if self.reverse.is_some_and(identity) {
            content.push_str(" (reverse)");
        }

        content.push('`');

        content.into_boxed_str()
    }
}

pub enum RegionTopKind {
    Global,
    Country {
        country_code: CompactString,
    },
    Region {
        country_code: CompactString,
        region_name: CompactString,
    },
}

async fn handle_autocomplete(
    command: &InteractionCommand,
    country: Option<&str>,
    region: &str,
) -> Result<()> {
    let Some(country) = country else {
        let choices = single_choice("Must specify country first");
        command.autocomplete(choices).await?;

        return Ok(());
    };

    let country_code = Countries::name(country)
        .to_code()
        .map(Cow::Borrowed)
        .or_else(|| (country.len() == 2).then_some(Countries::code(country).uppercase()));

    let Some(country_code) = country_code else {
        let choices = single_choice("Invalid country");
        command.autocomplete(choices).await?;

        return Ok(());
    };

    let region = region.cow_to_ascii_lowercase();

    let mut choices = match Context::redis().country_regions().await? {
        RedisData::Original(country_regions) => {
            let Some(regions) = country_regions.get(country_code.as_ref()) else {
                let choices = single_choice("No regions for specified country");
                command.autocomplete(choices).await?;

                return Ok(());
            };

            gather_choices(regions, &region)
        }
        RedisData::Archive(country_regions) => {
            let Some(regions) = country_regions.get(country_code.as_ref()) else {
                let choices = single_choice("No regions for specified country");
                command.autocomplete(choices).await?;

                return Ok(());
            };

            gather_choices(regions, &region)
        }
    };

    choices.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    command.autocomplete(choices).await?;

    Ok(())
}

fn single_choice(name: &str) -> Vec<CommandOptionChoice> {
    let choice = CommandOptionChoice {
        name: name.to_owned(),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(name.to_owned()),
    };

    vec![choice]
}

fn gather_choices<Code, Name>(
    regions: &impl RegionsExt<Code, Name>,
    region: &str,
) -> Vec<CommandOptionChoice>
where
    Code: AsRef<str>,
    Name: AsRef<str>,
{
    regions
        .iter()
        .filter_map(|(code, name)| new_choice(code.as_ref(), name.as_ref(), region))
        .take(25)
        .collect()
}

fn new_choice(code: &str, name: &str, region: &str) -> Option<CommandOptionChoice> {
    if !name.cow_to_ascii_lowercase().contains(region) && !region_in_code(code, region) {
        return None;
    };

    Some(CommandOptionChoice {
        name: name.to_owned(),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(name.to_owned()),
    })
}

fn region_in_code(code: &str, region: &str) -> bool {
    // Region codes are always(?) prefixed with "{country code}-"
    code.split_once('-')
        .map_or(code, |(_, suffix)| suffix)
        .cow_to_ascii_lowercase()
        .contains(region)
}

trait RegionsExt<Code, Name> {
    type Iter<'a>: Iterator<Item = (&'a Code, &'a Name)>
    where
        Code: 'a,
        Name: 'a,
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_>;
}

impl<Code, Name> RegionsExt<Code, Name> for HashMap<Code, Name> {
    type Iter<'a>
        = std::collections::hash_map::Iter<'a, Code, Name>
    where
        Code: 'a,
        Name: 'a,
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

impl<Code, Name> RegionsExt<Code, Name> for ArchivedHashMap<Code, Name> {
    type Iter<'a>
        = rkyv::collections::hash_map::Iter<'a, Code, Name>
    where
        Code: 'a,
        Name: 'a,
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

impl<'q> Searchable<ScoresCriteria<'q>>
    for (
        GameMode,
        &'_ DbTopScore,
        &'_ HashMap<u32, DbScoreBeatmap, IntHasher>,
        &'_ HashMap<u32, DbScoreBeatmapset, IntHasher>,
    )
{
    fn matches(&self, criteria: &FilterCriteria<ScoresCriteria<'q>>) -> bool {
        let (mode, score, maps, mapsets) = *self;
        let mut matches = true;

        matches &= criteria.combo.contains(score.max_combo);
        matches &= criteria.miss.contains(score.statistics.miss);
        matches &= criteria.score.contains(score.score);
        matches &= criteria.date.contains(score.ended_at.date());

        if !criteria.stars.is_empty() {
            let Some(stars) = score.stars else {
                return false;
            };
            matches &= criteria.stars.contains(stars);
        }

        if !criteria.pp.is_empty() {
            matches &= criteria.pp.contains(score.pp);
        }

        if !matches
            || (criteria.ar.is_empty()
                && criteria.cs.is_empty()
                && criteria.hp.is_empty()
                && criteria.od.is_empty()
                && criteria.length.is_empty()
                && criteria.bpm.is_empty()
                && criteria.version.is_empty()
                && criteria.artist.is_empty()
                && criteria.title.is_empty()
                && criteria.ranked_date.is_empty()
                && !criteria.has_search_terms())
        {
            return matches;
        }

        let Some(map) = maps.get(&score.map_id) else {
            return false;
        };

        let attrs = BeatmapAttributesBuilder::default()
            .ar(map.ar, false)
            .cs(map.cs, false)
            .hp(map.hp, false)
            .od(map.od, false)
            .mods(score.mods)
            .mode((mode as u8).into(), false)
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

        let Some(mapset) = mapsets.get(&map.mapset_id) else {
            return false;
        };

        if !criteria.ranked_date.is_empty() {
            let Some(datetime) = mapset.ranked_date else {
                return false;
            };
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
    }
}
