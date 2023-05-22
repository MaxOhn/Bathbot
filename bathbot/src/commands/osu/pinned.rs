use std::{
    cmp::{Ordering, Reverse},
    collections::HashMap,
    fmt::Write,
    sync::Arc,
};

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::{rosu_v2::user::User, ScoreSlim};
use bathbot_psql::model::configs::{GuildConfig, ListSize, MinimizedPp, ScoreSize};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    osu::ModSelection,
    IntHasher,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{
        GameMode, Grade, OsuError,
        RankStatus::{Approved, Loved, Qualified, Ranked},
        Score,
    },
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found, HasMods, ModsResult, ScoreOrder, TopEntry};
use crate::{
    active::{
        impls::{TopPagination, TopScoreEdit},
        ActiveMessages,
    },
    commands::GameModeOption,
    core::commands::CommandOrigin,
    manager::redis::{
        osu::{UserArgs, UserArgsSlim},
        RedisData,
    },
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, Searchable},
        InteractionCommandExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, HasMods, HasName, SlashCommand)]
#[command(name = "pinned", desc = "Display the user's pinned scores")]
pub struct Pinned {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<String>,
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
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Size of the embed",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
    size: Option<ListSize>,
}

async fn slash_pinned(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Pinned::from_interaction(command.input_data())?;

    pinned(ctx, (&mut command).into(), args).await
}

async fn pinned(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Pinned) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let msg_owner = orig.user_id()?;

    let mut config = match ctx.user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let (guild_score_size, guild_list_size, guild_minimized_pp) = match orig.guild_id() {
        Some(guild_id) => {
            let f =
                |config: &GuildConfig| (config.score_size, config.list_size, config.minimized_pp);

            ctx.guild_config().peek(guild_id, f).await
        }
        None => (None, None, None),
    };

    let list_size = args
        .size
        .or(config.list_size)
        .or(guild_list_size)
        .unwrap_or_default();

    let size_single = matches!(list_size, ListSize::Single);

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let (user_args, user_opt) = match UserArgs::rosu_id(&ctx, &user_id).await.mode(mode) {
        UserArgs::Args(args) => (args, None),
        UserArgs::User { user, mode } => (
            UserArgsSlim::user_id(user.user_id).mode(mode),
            Some(RedisData::Original(*user)),
        ),
        UserArgs::Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        UserArgs::Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let missing_user = user_opt.is_none();

    let scores_manager = ctx.osu_scores();
    let redis = ctx.redis();
    let pinned_fut = scores_manager.pinned().limit(100).exec(user_args);

    let top100_fut = async {
        if matches!(list_size, ListSize::Single) {
            scores_manager.top().limit(100).exec(user_args).await
        } else {
            Ok(Vec::new())
        }
    };

    let user_fut = async {
        if missing_user {
            redis.osu_user_from_args(user_args).await.map(Some)
        } else {
            Ok(None)
        }
    };

    let (pinned, top100, user) = match tokio::try_join!(pinned_fut, top100_fut, user_fut) {
        Ok((pinned, top100, user)) => (pinned, top100, user.or(user_opt).expect("missing user")),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or prepare scores");

            return Err(err);
        }
    };

    let entries =
        match process_scores(&ctx, pinned, &args, mods.as_ref(), &top100, size_single).await {
            Ok(entries) => entries,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to process scores"));
            }
        };

    let username = user.username();

    if let [score] = &entries[..] {
        let score_size = config.score_size.or(guild_score_size).unwrap_or_default();

        let minimized_pp = config
            .minimized_pp
            .or(guild_minimized_pp)
            .unwrap_or_default();

        let content = write_content(username, &args, 1, mods);

        single_embed(ctx, orig, user, score, score_size, minimized_pp, content).await
    } else {
        let content = write_content(username, &args, entries.len(), mods);
        let sort_by = args.sort.unwrap_or(ScoreOrder::Pp).into(); // TopOrder::Pp does not show anything

        let minimized_pp = config
            .minimized_pp
            .or(guild_minimized_pp)
            .unwrap_or_default();

        let pagination = TopPagination::builder()
            .user(user)
            .mode(mode)
            .entries(entries.into_boxed_slice())
            .sort_by(sort_by)
            .farm(HashMap::with_hasher(IntHasher))
            .list_size(list_size)
            .minimized_pp(minimized_pp)
            .content(content.unwrap_or_default())
            .msg_owner(msg_owner)
            .build();

        ActiveMessages::builder(pagination)
            .start_by_update(true)
            .begin(ctx, orig)
            .await
    }
}

async fn process_scores(
    ctx: &Context,
    pinned: Vec<Score>,
    args: &Pinned,
    mods: Option<&ModSelection>,
    top100: &[Score],
    size_single: bool,
) -> Result<Vec<TopEntry>> {
    let filter_criteria = args.query.as_deref().map(FilterCriteria::new);

    let mut entries = Vec::new();

    let maps_id_checksum = pinned
        .iter()
        .filter(|score| match filter_criteria {
            Some(ref criteria) => score.matches(criteria),
            None => true,
        })
        .filter(|score| match mods {
            None => true,
            Some(selection) => selection.filter_score(score),
        })
        .filter_map(|score| score.map.as_ref())
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let maps = ctx.osu_map().maps(&maps_id_checksum).await?;

    for (mut i, score) in pinned.into_iter().enumerate() {
        let Some(mut map) = maps.get(&score.map_id).cloned() else { continue };
        map.convert_mut(score.mode);

        let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods.bits());
        let attrs = calc.difficulty().await;
        let stars = attrs.stars() as f32;
        let max_combo = attrs.max_combo() as u32;

        let max_pp = match score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
        {
            Some(pp) => pp,
            None => calc.performance().await.pp() as f32,
        };

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);

        if size_single {
            i = top100
                .iter()
                .position(|s| score.is_eq(s))
                .unwrap_or(usize::MAX);
        }

        let entry = TopEntry {
            original_idx: i,
            score,
            map,
            max_pp,
            stars,
            max_combo,
        };

        entries.push(entry);
    }

    match args.sort {
        None => {}
        Some(ScoreOrder::Acc) => entries.sort_by(|a, b| {
            b.score
                .accuracy
                .partial_cmp(&a.score.accuracy)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Bpm) => entries.sort_by(|a, b| {
            b.map
                .bpm()
                .partial_cmp(&a.map.bpm())
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Combo) => entries.sort_by_key(|entry| Reverse(entry.score.max_combo)),
        Some(ScoreOrder::Date) => entries.sort_by_key(|entry| Reverse(entry.score.ended_at)),
        Some(ScoreOrder::Length) => {
            entries.sort_by(|a, b| {
                let a_len = a.map.seconds_drain() as f32 / a.score.mods.clock_rate().unwrap_or(1.0);
                let b_len = b.map.seconds_drain() as f32 / b.score.mods.clock_rate().unwrap_or(1.0);

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            });
        }
        Some(ScoreOrder::Misses) => entries.sort_by(|a, b| {
            b.score
                .statistics
                .count_miss
                .cmp(&a.score.statistics.count_miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.count_miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.count_miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        Some(ScoreOrder::Pp) => entries.sort_by(|a, b| {
            b.score
                .pp
                .partial_cmp(&a.score.pp)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::RankedDate) => {
            entries.sort_by_key(|entry| Reverse(entry.map.ranked_date()))
        }
        Some(ScoreOrder::Score) => entries.sort_by_key(|entry| Reverse(entry.score.score)),
        Some(ScoreOrder::Stars) => {
            entries.sort_by(|a, b| b.stars.partial_cmp(&a.stars).unwrap_or(Ordering::Equal))
        }
    }

    Ok(entries)
}

async fn single_embed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    user: RedisData<User>,
    entry: &TopEntry,
    score_size: ScoreSize,
    minimized_pp: MinimizedPp,
    content: Option<String>,
) -> Result<()> {
    let user_id = user.user_id();

    // Get indices of score in user top100 and map top50
    let (personal_idx, global_idx) = match entry.map.status() {
        Ranked | Loved | Qualified | Approved => {
            let user_args = UserArgsSlim::user_id(user_id).mode(entry.score.mode);
            let best_fut = ctx.osu_scores().top().limit(100).exec(user_args);

            // TODO: Add .limit(50) when supported by osu!api
            let global_fut = ctx.osu().beatmap_scores(entry.map.map_id());
            let (best_res, global_res) = tokio::join!(best_fut, global_fut);

            let personal_idx = match best_res {
                Ok(scores) => scores.iter().position(|s| entry.score.is_eq(s)),
                Err(err) => {
                    warn!(?err, "Failed to get top scores");

                    None
                }
            };

            let global_idx = match global_res {
                Ok(scores) => scores
                    .iter()
                    .position(|s| s.user_id == user_id && entry.score.is_eq(s)),
                Err(err) => {
                    warn!(?err, "Failed to get global scores");

                    None
                }
            };

            (personal_idx, global_idx)
        }
        _ => (None, None),
    };

    let active_msg_fut = TopScoreEdit::create(
        &ctx,
        &user,
        entry,
        personal_idx,
        global_idx,
        minimized_pp,
        score_size,
        content,
    );

    ActiveMessages::builder(active_msg_fut.await)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

fn write_content(
    name: &str,
    args: &Pinned,
    amount: usize,
    mods: Option<ModSelection>,
) -> Option<String> {
    if args.query.is_some() || mods.is_some() {
        Some(content_with_condition(args, amount, mods))
    } else if let Some(sort_by) = args.sort {
        let genitive = if name.ends_with('s') { "" } else { "s" };

        let content = match sort_by {
            ScoreOrder::Acc => format!("`{name}`'{genitive} pinned scores sorted by accuracy:"),
            ScoreOrder::Bpm => format!("`{name}`'{genitive} pinned scores sorted by BPM:"),
            ScoreOrder::Combo => format!("`{name}`'{genitive} pinned scores sorted by combo:"),
            ScoreOrder::Date => format!("Most recent pinned scores of `{name}`:"),
            ScoreOrder::Length => format!("`{name}`'{genitive} pinned scores sorted by length:"),
            ScoreOrder::Misses => {
                format!("`{name}`'{genitive} pinned scores sorted by miss count:")
            }
            ScoreOrder::Pp => format!("`{name}`'{genitive} pinned scores sorted by pp"),
            ScoreOrder::RankedDate => {
                format!("`{name}`'{genitive} pinned scores sorted by ranked date:")
            }
            ScoreOrder::Score => format!("`{name}`'{genitive} pinned scores sorted by score"),
            ScoreOrder::Stars => format!("`{name}`'{genitive} pinned scores sorted by stars"),
        };

        Some(content)
    } else if amount == 0 {
        Some(format!("`{name}` has not pinned any scores"))
    } else if amount == 1 {
        Some(format!("`{name}` has pinned 1 score:"))
    } else {
        None
    }
}

fn content_with_condition(args: &Pinned, amount: usize, mods: Option<ModSelection>) -> String {
    let mut content = String::with_capacity(64);

    match args.sort {
        Some(ScoreOrder::Acc) => content.push_str("`Order: Accuracy`"),
        Some(ScoreOrder::Bpm) => content.push_str("`Order: BPM`"),
        Some(ScoreOrder::Combo) => content.push_str("`Order: Combo`"),
        Some(ScoreOrder::Date) => content.push_str("`Order: Date`"),
        Some(ScoreOrder::Length) => content.push_str("`Order: Length`"),
        Some(ScoreOrder::Misses) => content.push_str("`Order: Miss count`"),
        Some(ScoreOrder::Pp) => content.push_str("`Order: Pp`"),
        Some(ScoreOrder::RankedDate) => content.push_str("`Order: Ranked date`"),
        Some(ScoreOrder::Score) => content.push_str("`Order: Score`"),
        Some(ScoreOrder::Stars) => content.push_str("`Order: Stars`"),
        None => {}
    }

    if let Some(selection) = mods {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let (pre, mods) = match selection {
            ModSelection::Include(mods) => ("Include ", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Exact(mods) => ("", mods),
        };

        let _ = write!(content, "`Mods: {pre}{mods}`");
    }

    if let Some(query) = args.query.as_deref() {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Query: {query}`");
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching pinned score{plural}:");

    content
}
