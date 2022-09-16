use std::{fmt::Write, sync::Arc};

use command_macros::{HasMods, HasName, SlashCommand};
use eyre::{Report, Result};
use hashbrown::HashMap;
use rosu_v2::prelude::{
    GameMode, GameMods, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{osu::UserArgs, GameModeOption},
    core::commands::CommandOrigin,
    database::{EmbedsSize, ListSize, MinimizedPp},
    embeds::TopSingleEmbed,
    pagination::{TopCondensedPagination, TopPagination, TopSinglePagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        hasher::SimpleBuildHasher,
        interaction::InteractionCommand,
        osu::ModSelection,
        query::{FilterCriteria, Searchable},
        InteractionCommandExt, MessageExt,
    },
    Context,
};

use super::{prepare_scores, require_link, HasMods, ModsResult, ScoreOrder};

#[derive(CommandModel, CreateCommand, HasMods, HasName, SlashCommand)]
#[command(name = "pinned")]
/// Display the user's pinned scores
pub struct Pinned {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    /// Choose how the scores should be ordered
    sort: Option<ScoreOrder>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
    ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
    While ar & co will be adjusted to mods, stars will not."
    )]
    /// Specify a search query containing artist, difficulty, AR, BPM, ...
    query: Option<String>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
    or `-mods!` for excluded mods.\n\
    Examples:\n\
    - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
    - `+hdhr!`: Scores must have exactly `HDHR`\n\
    - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
    - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
    #[command(help = "Size of the embed.\n\
      `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
      The default can be set with the `/config` command.")]
    /// Size of the embed
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

    let mut config = match ctx.user_config(orig.user_id()?).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match config.osu.take() {
            Some(osu) => osu.into_username(),
            None => return require_link(&ctx, &orig).await,
        },
    };

    // Retrieve the user and their top scores
    let mut user_args = UserArgs::new(&name, mode);

    let result = if let Some(alt_name) = user_args.whitespaced_name() {
        match ctx.redis().osu_user(&user_args).await {
            Ok(user) => {
                let scores_fut = ctx
                    .osu()
                    .user_scores(user_args.name)
                    .pinned()
                    .mode(mode)
                    .limit(100);

                prepare_scores(&ctx, scores_fut)
                    .await
                    .map(|scores| (user, scores))
            }
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;
                let redis = ctx.redis();

                let user_fut = redis.osu_user(&user_args);

                let scores_fut = ctx
                    .osu()
                    .user_scores(user_args.name)
                    .pinned()
                    .mode(mode)
                    .limit(100);

                tokio::try_join!(user_fut, prepare_scores(&ctx, scores_fut))
            }
            Err(err) => Err(err),
        }
    } else {
        let redis = ctx.redis();
        let user_fut = redis.osu_user(&user_args);

        let scores_fut = ctx
            .osu()
            .user_scores(user_args.name)
            .pinned()
            .mode(mode)
            .limit(100);

        tokio::try_join!(user_fut, prepare_scores(&ctx, scores_fut))
    };

    let (mut user, mut scores) = match result {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user or prepare scores");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Filter scores according to query
    filter_scores(&ctx, &mut scores, &args, mods).await;

    if let [score] = &scores[..] {
        let embeds_size = match (config.score_size, orig.guild_id()) {
            (Some(size), _) => size,
            (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
            (None, None) => EmbedsSize::default(),
        };

        let minimized_pp = match (config.minimized_pp, orig.guild_id()) {
            (Some(pp), _) => pp,
            (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
            (None, None) => MinimizedPp::default(),
        };

        let content = write_content(&name, &args, 1, mods);
        single_embed(ctx, orig, user, score, embeds_size, minimized_pp, content).await?;
    } else {
        let list_size = match args.size {
            Some(size) => size,
            None => match (config.list_size, orig.guild_id()) {
                (Some(size), _) => size,
                (None, Some(guild)) => ctx.guild_list_size(guild).await,
                (None, None) => ListSize::default(),
            },
        };

        let content = write_content(&name, &args, scores.len(), mods);
        let sort_by = args.sort.unwrap_or(ScoreOrder::Pp).into(); // TopOrder::Pp does not show anything
        let scores: Vec<_> = scores.into_iter().enumerate().collect();
        let farm = HashMap::with_hasher(SimpleBuildHasher);

        match list_size {
            ListSize::Condensed => {
                TopCondensedPagination::builder(user, scores, sort_by, farm)
                    .content(content.unwrap_or_default())
                    .start_by_update()
                    .defer_components()
                    .start(ctx, orig)
                    .await?;
            }
            ListSize::Detailed => {
                TopPagination::builder(user, scores, sort_by, farm)
                    .content(content.unwrap_or_default())
                    .start_by_update()
                    .defer_components()
                    .start(ctx, orig)
                    .await?;
            }
            ListSize::Single => {
                let minimized_pp = match (config.minimized_pp, orig.guild_id()) {
                    (Some(pp), _) => pp,
                    (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
                    (None, None) => MinimizedPp::default(),
                };

                TopSinglePagination::builder(user, scores, minimized_pp)
                    .content(content.unwrap_or_default())
                    .start_by_update()
                    .defer_components()
                    .start(ctx, orig)
                    .await?;
            }
        }
    }

    Ok(())
}

async fn filter_scores(
    ctx: &Context,
    scores: &mut Vec<Score>,
    args: &Pinned,
    mods: Option<ModSelection>,
) {
    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);

        scores.retain(|score| score.matches(&criteria));
    }

    match mods {
        Some(ModSelection::Include(GameMods::NoMod)) => {
            scores.retain(|score| score.mods.is_empty())
        }
        Some(ModSelection::Include(mods)) => {
            scores.retain(|score| score.mods.intersection(mods) == mods)
        }
        Some(ModSelection::Exact(mods)) => scores.retain(|score| score.mods == mods),
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores.retain(|score| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|score| score.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    if let Some(sort_by) = args.sort {
        sort_by.apply(ctx, scores).await;
    }
}

async fn single_embed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    user: User,
    score: &Score,
    embeds_size: EmbedsSize,
    minimized_pp: MinimizedPp,
    content: Option<String>,
) -> Result<()> {
    let map = score.map.as_ref().unwrap();

    // Get indices of score in user top100 and map top50
    let (personal_idx, global_idx) = match map.status {
        Ranked | Loved | Qualified | Approved => {
            let best_fut = ctx
                .osu()
                .user_scores(user.user_id)
                .mode(score.mode)
                .limit(100);

            let global_fut = ctx.osu().beatmap_scores(map.map_id); // TODO: Add .limit(50) when supported by osu!api
            let (best_result, global_result) = tokio::join!(best_fut, global_fut);

            let personal_idx = match best_result {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get top scores");
                    warn!("{report:?}");

                    None
                }
            };

            let global_idx = match global_result {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(err) => {
                    let report = Report::new(err).wrap_err("Failed to get global scores");
                    warn!("{report:?}");

                    None
                }
            };

            (personal_idx, global_idx)
        }
        _ => (None, None),
    };

    let embed_data =
        TopSingleEmbed::new(&user, score, personal_idx, global_idx, minimized_pp, &ctx).await?;

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let mut builder = MessageBuilder::new().embed(embed_data.into_minimized());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let mut builder = MessageBuilder::new().embed(embed_data.as_maximized());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            let response = orig.create_message(&ctx, &builder).await?.model().await?;
            ctx.store_msg(response.id);

            // Minimize embed after delay
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let builder = embed_data.into_minimized().into();

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("Failed to minimize embed");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let mut builder = MessageBuilder::new().embed(embed_data.as_maximized());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
    }

    Ok(())
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
