use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, PPMissingEmbed},
    util::{
        builder::MessageBuilder, constants::OSU_API_ISSUE, interaction::InteractionCommand,
        matcher, ChannelExt, InteractionCommandExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "pp")]
/// How many pp is a user missing to reach the given amount?
pub struct Pp<'a> {
    /// Specify a target total pp amount
    pp: f32,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 0.0)]
    /// Fill a top100 with scores of this many pp until the target total pp are reached
    each: Option<f32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

impl<'m> Pp<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut pp = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                pp = Some(num);
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Ok(Self {
            pp: pp.ok_or("You need to provide a decimal number")?,
            mode,
            name,
            each: None,
            discord,
        })
    }
}

async fn slash_pp(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Pp::from_interaction(command.input_data())?;

    pp(ctx, (&mut command).into(), args).await
}

#[command]
#[desc("How many pp are missing to reach the given amount?")]
#[help(
    "Calculate what score a user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[group(Osu)]
pub async fn prefix_pp(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Pp::args(None, args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp are missing to reach the given amount?")]
#[help(
    "Calculate what score a mania user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[alias("ppm")]
#[group(Mania)]
pub async fn prefix_ppmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Pp::args(Some(GameModeOption::Mania), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp are missing to reach the given amount?")]
#[help(
    "Calculate what score a taiko user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[alias("ppt")]
#[group(Taiko)]
pub async fn prefix_pptaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Pp::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many pp are missing to reach the given amount?")]
#[help(
    "Calculate what score a ctb user is missing to \
     reach the given total pp amount"
)]
#[usage("[username] [number]")]
#[example("badewanne3 8000")]
#[aliases("ppc", "ppcatch")]
#[group(Catch)]
pub async fn prefix_ppctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Pp::args(Some(GameModeOption::Catch), args) {
        Ok(args) => pp(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn pp(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Pp<'_>) -> Result<()> {
    let (name, mode) = name_mode!(ctx, orig, args);

    let Pp { pp, each, .. } = args;

    if pp < 0.0 {
        return orig.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return orig.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100);
    let user_scores_fut = get_user_and_scores(&ctx, user_args, &score_args);
    let rank_fut = ctx.psql().approx_rank_from_pp(pp, mode);

    let (user_scores_result, rank_result) = tokio::join!(user_scores_fut, rank_fut);

    let (mut user, mut scores) = match user_scores_result {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user or scores");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    let rank = match rank_result {
        Ok(rank_pp) => Some(rank_pp),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get rank pp"));

            None
        }
    };

    // Process user and their top scores for tracking
    #[cfg(feature = "osutracking")]
    crate::tracking::process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Accumulate all necessary data
    let embed_data = PPMissingEmbed::new(user, &mut scores, pp, rank, each);

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
