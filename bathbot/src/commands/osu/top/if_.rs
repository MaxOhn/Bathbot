use std::{borrow::Cow, fmt::Write, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::round,
    osu::ModSelection,
    CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameModIntermode, GameMode, GameMods, GameModsIntermode, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    active::{impls::TopIfPagination, ActiveMessages},
    commands::{
        osu::{require_link, user_not_found},
        GameModeOption,
    },
    core::{
        commands::{prefix::Args, CommandOrigin},
        ContextExt,
    },
    manager::{redis::osu::UserArgs, OsuMap},
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, IFilterCriteria, Searchable, TopCriteria},
        ChannelExt, InteractionCommandExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "topif",
    desc = "How the top plays would look like with different mods"
)]
pub struct TopIf<'a> {
    #[command(
        desc = "Specify mods (`+mods` to insert them, `+mods!` to replace, `-mods!` to remove)",
        help = "Specify how the top score mods should be adjusted.\n\
        Mods must be given as `+mods` to included them everywhere, `+mods!` to replace them exactly, \
        or `-mods!` to excluded them everywhere.\n\
        Examples:\n\
        - `+hd`: Add `HD` to all scores\n\
        - `+hdhr!`: Make all scores `HDHR` scores\n\
        - `+nm!`: Make all scores nomod scores\n\
        - `-ezhd!`: Remove both `EZ` and `HD` from all scores"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopIfScoreOrder>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, Default, CommandOption, CreateOption, Eq, PartialEq)]
pub enum TopIfScoreOrder {
    #[default]
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "PP delta", value = "pp_delta")]
    PpDelta,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

async fn slash_topif(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = TopIf::from_interaction(command.input_data())?;

    topif(ctx, (&mut command).into(), args).await
}

impl<'m> TopIf<'m> {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want add mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to remove mods, specify it e.g. as `-hdnf!`.";

    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut mods = None;

        for arg in args.take(2) {
            if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Ok(Self {
            mods: mods.ok_or(Self::ERR_PARSE_MODS)?,
            mode,
            name,
            query: None,
            sort: None,
            discord,
        })
    }
}

#[command]
#[desc("Display a user's top plays with(out) the given mods")]
#[help(
    "Display how a user's top plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[alias("ti")]
#[group(Osu)]
async fn prefix_topif(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(None, args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top taiko plays with(out) the given mods")]
#[help(
    "Display how a user's top taiko plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[alias("tit")]
#[group(Taiko)]
async fn prefix_topiftaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top ctb plays with(out) the given mods")]
#[help(
    "Display how a user's top ctb plays would look like with the given mods.\n\
    As for all other commands with mods input, you can specify them as follows:\n  \
    - `+mods` to include the mod(s) into all scores\n  \
    - `+mods!` to make all scores have exactly those mods\n  \
    - `-mods!` to remove all these mods from all scores"
)]
#[usage("[username] [mods")]
#[examples("badewanne3 -hd!", "+hdhr!", "whitecat +hddt")]
#[aliases("tic", "topifcatch")]
#[group(Catch)]
async fn prefix_topifctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match TopIf::args(Some(GameModeOption::Catch), args) {
        Ok(args) => topif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn topif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TopIf<'_>) -> Result<()> {
    let mods = match matcher::get_mods(&args.mods) {
        Some(mods) => mods,
        None => return orig.error(&ctx, TopIf::ERR_PARSE_MODS).await,
    };

    let owner = orig.user_id()?;
    let config = ctx.user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let mode = match args.mode.map(GameMode::from).or(config.mode) {
        None | Some(GameMode::Mania) => GameMode::Osu,
        Some(mode) => mode,
    };

    if let Err(content) = mods.clone().validate(mode) {
        return orig.error(&ctx, content).await;
    }

    let legacy_scores = match config.legacy_scores {
        Some(legacy_scores) => legacy_scores,
        None => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.legacy_scores)
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
    let scores_fut = ctx
        .osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .fold(0.0, |sum, weight| sum + weight.pp);

    let bonus_pp = user.stats().pp() - actual_pp;
    let sort = args.sort.unwrap_or_default();
    let content = get_content(user.username(), mode, &mods, args.query.as_deref(), sort);

    let mut entries = match process_scores(ctx.cloned(), scores, mods, mode, sort).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to modify scores"));
        }
    };

    // Calculate adjusted pp
    let adjusted_pp: f32 = entries.iter().zip(0..).fold(0.0, |sum, (entry, i)| {
        sum + entry.score.pp * 0.95_f32.powi(i)
    });

    // Process query afterwards so that total pp is calculated with *all* scores
    if let Some(query) = args.query.as_deref() {
        let criteria = TopCriteria::create(query);
        entries.retain(|entry| entry.matches(&criteria));
    }

    let final_pp = round(bonus_pp + adjusted_pp);

    let rank = match ctx.approx().rank(final_pp, mode).await {
        Ok(rank) => Some(rank),
        Err(err) => {
            warn!(?err, "Failed to get rank from pp");

            None
        }
    };

    // Accumulate all necessary data
    let pre_pp = user.stats().pp();

    let pagination = TopIfPagination::builder()
        .user(user)
        .entries(entries.into_boxed_slice())
        .mode(mode)
        .pre_pp(pre_pp)
        .post_pp(final_pp)
        .rank(rank)
        .content(content.into_boxed_str())
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

pub struct TopIfEntry {
    pub original_idx: usize,
    pub old_pp: f32,
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub stars: f32,
    pub max_pp: f32,
    pub max_combo: u32,
}

impl TopIfEntry {
    pub fn pp_delta(&self) -> f32 {
        (self.score.pp - self.old_pp).abs()
    }
}

impl<'q> Searchable<TopCriteria<'q>> for TopIfEntry {
    fn matches(&self, criteria: &FilterCriteria<TopCriteria<'q>>) -> bool {
        let mut matches = true;

        matches &= criteria.combo.contains(self.score.max_combo);
        matches &= criteria.miss.contains(self.score.statistics.count_miss);
        matches &= criteria.score.contains(self.score.score);
        matches &= criteria.date.contains(self.score.ended_at.date());
        matches &= criteria.stars.contains(self.stars);
        matches &= criteria.pp.contains(self.score.pp);
        matches &= criteria.acc.contains(self.score.accuracy);

        if !criteria.ranked_date.is_empty() {
            let Some(datetime) = self.map.ranked_date() else {
                return false;
            };
            matches &= criteria.ranked_date.contains(datetime.date());
        }

        let attrs = self.map.attributes().mods(self.score.mods.bits()).build();

        matches &= criteria.ar.contains(attrs.ar as f32);
        matches &= criteria.cs.contains(attrs.cs as f32);
        matches &= criteria.hp.contains(attrs.hp as f32);
        matches &= criteria.od.contains(attrs.od as f32);

        let keys = [
            (GameModIntermode::OneKey, 1.0),
            (GameModIntermode::TwoKeys, 2.0),
            (GameModIntermode::ThreeKeys, 3.0),
            (GameModIntermode::FourKeys, 4.0),
            (GameModIntermode::FiveKeys, 5.0),
            (GameModIntermode::SixKeys, 6.0),
            (GameModIntermode::SevenKeys, 7.0),
            (GameModIntermode::EightKeys, 8.0),
            (GameModIntermode::NineKeys, 9.0),
            (GameModIntermode::TenKeys, 10.0),
        ]
        .into_iter()
        .find_map(|(gamemod, keys)| self.score.mods.contains_intermode(gamemod).then_some(keys))
        .unwrap_or(attrs.cs as f32);

        matches &= self.map.mode() != GameMode::Mania || criteria.keys.contains(keys);

        if !matches
            || (criteria.length.is_empty()
                && criteria.bpm.is_empty()
                && criteria.artist.is_empty()
                && criteria.creator.is_empty()
                && criteria.version.is_empty()
                && criteria.title.is_empty()
                && !criteria.has_search_terms())
        {
            return matches;
        }

        let clock_rate = attrs.clock_rate as f32;
        matches &= criteria
            .length
            .contains(self.map.seconds_drain() as f32 / clock_rate);
        matches &= criteria.bpm.contains(self.map.bpm() * clock_rate);

        if criteria.artist.is_empty()
            && criteria.creator.is_empty()
            && criteria.title.is_empty()
            && criteria.version.is_empty()
            && !criteria.has_search_terms()
        {
            return matches;
        }

        let artist = self.map.artist().cow_to_ascii_lowercase();
        matches &= criteria.artist.matches(&artist);

        let creator = self.map.creator().cow_to_ascii_lowercase();
        matches &= criteria.creator.matches(&creator);

        let version = self.map.version().cow_to_ascii_lowercase();
        matches &= criteria.version.matches(&version);

        let title = self.map.title().cow_to_ascii_lowercase();
        matches &= criteria.title.matches(&title);

        if matches && criteria.has_search_terms() {
            let terms = [artist, creator, version, title];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)))
        }

        matches
    }
}

async fn process_scores(
    ctx: Arc<Context>,
    scores: Vec<Score>,
    mut arg_mods: ModSelection,
    mode: GameMode,
    sort: TopIfScoreOrder,
) -> Result<Vec<TopIfEntry>> {
    let mut entries = Vec::with_capacity(scores.len());

    let maps_id_checksum = scores
        .iter()
        .map(|score| {
            (
                score.map_id as i32,
                score.map.as_ref().and_then(|map| map.checksum.as_deref()),
            )
        })
        .collect();

    let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;

    match &mut arg_mods {
        ModSelection::Exact(mods) | ModSelection::Include(mods) if mods.is_empty() => {
            *mods = GameModsIntermode::new();
        }
        ModSelection::Exclude(mods) => {
            if mods.contains(GameModIntermode::DoubleTime) {
                *mods |= GameModIntermode::Nightcore;
            }

            if mods.contains(GameModIntermode::SuddenDeath) {
                *mods |= GameModIntermode::Perfect;
            }
        }
        ModSelection::Exact(_) | ModSelection::Include(_) => {}
    }

    let converted_mods = arg_mods
        .as_mods()
        .to_owned()
        .with_mode(mode)
        .expect("mods have been validated before");

    for (mut score, i) in scores.into_iter().zip(1..) {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };
        map = map.convert(score.mode);

        let changed = match &arg_mods {
            ModSelection::Include(mods) if mods.is_empty() => {
                let changed = !score.mods.is_empty();
                score.mods = GameMods::new();

                changed
            }
            ModSelection::Exact(_) => {
                let changed = score.mods != converted_mods;
                score.mods = converted_mods.clone();

                changed
            }
            ModSelection::Exclude(mods) => {
                let changed = score.mods.contains_any(mods.iter());
                score.mods.remove_all_intermode(mods.iter());

                changed
            }
            ModSelection::Include(mods) => {
                let mut changed = false;

                if mods.contains(GameModIntermode::DoubleTime)
                    || mods.contains(GameModIntermode::Nightcore)
                {
                    changed |= score.mods.remove_intermode(GameModIntermode::HalfTime);
                }

                if mods.contains(GameModIntermode::HalfTime) {
                    changed |= score.mods.remove_intermode(GameModIntermode::DoubleTime);
                    changed |= score.mods.remove_intermode(GameModIntermode::Nightcore);
                }

                if mods.contains(GameModIntermode::HardRock) {
                    changed |= score.mods.remove_intermode(GameModIntermode::Easy);
                }

                if mods.contains(GameModIntermode::Easy) {
                    changed |= score.mods.remove_intermode(GameModIntermode::HardRock);
                }

                changed |= !mods
                    .iter()
                    .all(|gamemod| score.mods.contains_intermode(gamemod));

                score.mods.extend(converted_mods.iter().cloned());

                changed
            }
        };

        if changed {
            score.grade = score.grade(Some(score.accuracy));
        }

        let mut calc = ctx.pp(&map).mode(score.mode).mods(&score.mods);
        let attrs = calc.performance().await;

        let old_pp = score.pp.expect("missing pp");

        let new_pp = if changed {
            calc.score(&score).performance().await.pp() as f32
        } else {
            old_pp
        };

        let entry = TopIfEntry {
            original_idx: i,
            score: ScoreSlim::new(score, new_pp),
            old_pp,
            map,
            stars: attrs.stars() as f32,
            max_pp: attrs.pp() as f32,
            max_combo: attrs.max_combo(),
        };

        entries.push(entry);
    }

    match sort {
        TopIfScoreOrder::Pp => entries.sort_unstable_by(|a, b| b.score.pp.total_cmp(&a.score.pp)),
        TopIfScoreOrder::PpDelta => entries.sort_unstable_by(|a, b| {
            b.pp_delta()
                .total_cmp(&a.pp_delta())
                .then_with(|| b.score.pp.total_cmp(&a.score.pp))
        }),
        TopIfScoreOrder::Stars => entries.sort_unstable_by(|a, b| {
            b.stars
                .total_cmp(&a.stars)
                .then_with(|| b.score.pp.total_cmp(&a.score.pp))
        }),
    }

    Ok(entries)
}

fn get_content(
    name: &str,
    mode: GameMode,
    mods: &ModSelection,
    query: Option<&str>,
    sort: TopIfScoreOrder,
) -> String {
    let mut content = match mods {
        ModSelection::Exact(mods) => format!(
            "`{name}`{plural} {mode}top100 with only `{mods}` scores",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        ModSelection::Exclude(mods) if !mods.is_empty() => {
            let mods: Vec<_> = mods.iter().collect();
            let len = mods.len();
            let mut mod_iter = mods.into_iter();
            let mut mod_str = String::with_capacity(len * 6 - 2);

            if let Some(first) = mod_iter.next() {
                let last = mod_iter.next_back();
                let _ = write!(mod_str, "`{first}`");

                for elem in mod_iter {
                    let _ = write!(mod_str, ", `{elem}`");
                }

                if let Some(last) = last {
                    let _ = match len {
                        2 => write!(mod_str, " and `{last}`"),
                        _ => write!(mod_str, ", and `{last}`"),
                    };
                }
            }
            format!(
                "`{name}`{plural} {mode}top100 without {mods}",
                plural = plural(name),
                mode = mode_str(mode),
                mods = mod_str
            )
        }
        ModSelection::Include(mods) if !mods.is_empty() => format!(
            "`{name}`{plural} {mode}top100 with `{mods}` inserted everywhere",
            plural = plural(name),
            mode = mode_str(mode),
        ),
        _ => format!(
            "`{name}`{plural} top {mode}scores",
            plural = plural(name),
            mode = mode_str(mode),
        ),
    };

    if let Some(query) = query {
        TopCriteria::create(query).display(&mut content);
    }

    content.push_str(" • `Order: ");

    let sort_str = match sort {
        TopIfScoreOrder::Pp => "New PP",
        TopIfScoreOrder::PpDelta => "PP delta",
        TopIfScoreOrder::Stars => "New stars",
    };

    content.push_str(sort_str);

    content.push('`');

    content
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Taiko => "taiko ",
        GameMode::Catch => "ctb ",
        GameMode::Mania => "mania ",
    }
}
