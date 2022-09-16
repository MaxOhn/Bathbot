use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    database::{ListSize, MinimizedPp},
    pagination::{TopCondensedPagination, TopPagination, TopSinglePagination},
    tracking::process_osu_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        hasher::SimpleBuildHasher,
        interaction::InteractionCommand,
        matcher, ChannelExt, CowUtils, InteractionCommandExt,
    },
    Context,
};

use super::{get_user, require_link, TopScoreOrder};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "mapper",
    help = "Count the top plays on maps of the given mapper.\n\
    It will try to consider guest difficulties so that if a map was created by someone else \
    but the given mapper made the guest diff, it will count.\n\
    Similarly, if the given mapper created the mapset but someone else guest diff'd, \
    it will not count.\n\
    This does not always work perfectly, especially for older maps but it's what the api provides."
)]
/// How often does the given mapper appear in top a user's top plays
pub struct Mapper<'a> {
    /// Specify a mapper username
    mapper: Cow<'a, str>,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
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

impl<'m> Mapper<'m> {
    fn args(
        mode: Option<GameModeOption>,
        mut args: Args<'m>,
        mapper: Option<&'static str>,
    ) -> Result<Self, &'static str> {
        let mapper = match mapper.or_else(|| args.next()) {
            Some(arg) => arg.into(),
            None => {
                let content = "You need to specify at least one osu! username for the mapper. \
                    If you're not linked, you must specify at least two names.";

                return Err(content);
            }
        };

        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Ok(Self {
            mapper,
            mode,
            name,
            discord,
            size: None,
        })
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[group(Osu)]
async fn prefix_mapper(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(None, args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a mania user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a mania user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[alias("mapperm")]
#[group(Mania)]
pub async fn prefix_mappermania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Mania), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[alias("mappert")]
#[group(Taiko)]
pub async fn prefix_mappertaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Taiko), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[aliases("mapperc", "mappercatch")]
#[group(Catch)]
async fn prefix_mapperctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Catch), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by Sotarks?")]
#[usage("[username]")]
#[example("badewanne3")]
#[group(Osu)]
pub async fn prefix_sotarks(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Osu), args, Some("sotarks")) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_mapper(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Mapper::from_interaction(command.input_data())?;

    mapper(ctx, (&mut command).into(), args).await
}

async fn mapper(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Mapper<'_>) -> Result<()> {
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

    let user = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match config.osu.take() {
            Some(osu) => osu.into_username(),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let mapper = args.mapper.cow_to_ascii_lowercase();
    let mapper_args = UserArgs::new(mapper.as_ref(), mode);
    let mapper_fut = get_user(&ctx, &mapper_args);

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(user.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let user_scores_fut = get_user_and_scores(&ctx, user_args, &score_args);

    let (mapper, mut user, mut scores) = match tokio::join!(mapper_fut, user_scores_fut) {
        (Ok(mapper), Ok((user, scores))) => (mapper, user, scores),
        (Err(OsuError::NotFound), _) => {
            let content = format!("Mapper with username `{mapper}` was not found");

            return orig.error(&ctx, content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = format!("User `{user}` was not found");

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get mapper, user, or scores");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let scores: Vec<_> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, score)| {
            score
                .map
                .as_ref()
                .map(|map| map.creator_id == mapper.user_id)
                .unwrap_or(false)
        })
        .collect();

    let mapper = mapper.username;

    // Accumulate all necessary data
    let content = match mapper.as_str() {
        "Sotarks" => {
            let amount = scores.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = user.username,
            );

            let to_push = match amount {
                0 => "I'm proud \\:)",
                1..=4 => "that's already too many...",
                5..=8 => "kinda sad \\:/",
                9..=15 => "pretty sad \\:(",
                16..=25 => "this is so sad \\:((",
                26..=35 => "this needs to stop",
                36..=49 => "that's a serious problem...",
                50 => "that's half. HALF.",
                51..=79 => "how do you sleep at night...",
                80..=99 => "i'm not even mad, that's just impressive",
                100 => "you did it. \"Congrats\".",
                _ => "wait how did you do that",
            };

            content.push_str(to_push);

            content
        }
        _ => format!(
            "{} of `{}`'{} top score maps were mapped by `{mapper}`",
            scores.len(),
            user.username,
            if user.username.ends_with('s') {
                ""
            } else {
                "s"
            },
        ),
    };

    let sort_by = TopScoreOrder::Pp;
    let farm = HashMap::with_hasher(SimpleBuildHasher);

    let list_size = match args.size {
        Some(size) => size,
        None => match (config.list_size, orig.guild_id()) {
            (Some(size), _) => size,
            (None, Some(guild)) => ctx.guild_list_size(guild).await,
            (None, None) => ListSize::default(),
        },
    };

    match list_size {
        ListSize::Condensed => {
            TopCondensedPagination::builder(user, scores, sort_by, farm)
                .content(content)
                .start_by_update()
                .defer_components()
                .start(ctx, orig)
                .await
        }
        ListSize::Detailed => {
            TopPagination::builder(user, scores, sort_by, farm)
                .content(content)
                .start_by_update()
                .defer_components()
                .start(ctx, orig)
                .await
        }
        ListSize::Single => {
            let minimized_pp = match (config.minimized_pp, orig.guild_id()) {
                (Some(pp), _) => pp,
                (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
                (None, None) => MinimizedPp::default(),
            };

            TopSinglePagination::builder(user, scores, minimized_pp)
                .content(content)
                .start_by_update()
                .defer_components()
                .start(ctx, orig)
                .await
        }
    }
}
