use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{constants::OSU_API_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::user_not_found;
use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, PpMissingEmbed},
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "pp",
    desc = "How many pp is a user missing to reach the given amount?"
)]
pub struct Pp<'a> {
    #[command(
        desc = "Specify a target total pp amount",
        help = "Specify a target total pp amount.\n\
        Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
        meaning the current total pp + the given value"
    )]
    pp: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 0.0,
        desc = "Fill a top100 with scores of this many pp until the target total pp are reached"
    )]
    each: Option<f32>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl<'m> Pp<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut pp = None;

        for arg in args.take(2) {
            if arg.parse::<f32>().is_ok() {
                pp = Some(Cow::Borrowed(arg));
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
    "Calculate what score a user is missing to reach the given total pp amount.\n\
    Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
    meaning the current total pp + the given value"
)]
#[usage("[username] [+][number]")]
#[example("badewanne3 8000", "+72.7")]
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
    "Calculate what score a mania user is missing to reach the given total pp amount.\n\
    Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
    meaning the current total pp + the given value"
)]
#[usage("[username] [+][number]")]
#[example("badewanne3 8000", "+72.7")]
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
    "Calculate what score a taiko user is missing to reach the given total pp amount.\n\
    Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
    meaning the current total pp + the given value"
)]
#[usage("[username] [+][number]")]
#[example("badewanne3 8000", "+72.7")]
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
    "Calculate what score a ctb user is missing to reach the given total pp amount.\n\
    Alternatively, prefix the value with a `+` so that it'll be interpreted as \"delta\" \
    meaning the current total pp + the given value"
)]
#[usage("[username] [+][number]")]
#[example("badewanne3 8000", "+72.7")]
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
    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    let Pp { pp, each, .. } = args;

    let Some(pp) = PpValue::parse(pp.as_ref()) else {
        let content = "Failed to parse pp. Be sure to specify a decimal number.";

        return orig.error(&ctx, content).await;
    };

    let pp_value = pp.value();

    if pp_value < 0.0 {
        return orig.error(&ctx, "The pp number must be non-negative").await;
    } else if pp_value > (i64::MAX / 1024) as f32 {
        return orig.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
    let scores_fut = ctx.osu_scores().top().limit(100).exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    let target_pp = match pp {
        PpValue::Raw(value) => value,
        PpValue::Delta(value) => match user {
            RedisData::Original(ref user) => user
                .statistics
                .as_ref()
                .map_or(value, |stats| stats.pp + value),
            RedisData::Archive(ref user) => user
                .statistics
                .as_ref()
                .map_or(value, |stats| stats.pp + value),
        },
    };

    let rank = match ctx.approx().rank(target_pp, mode).await {
        Ok(rank_pp) => Some(rank_pp),
        Err(err) => {
            warn!(?err, "Failed to get rank pp");

            None
        }
    };

    // Accumulate all necessary data
    let embed_data = PpMissingEmbed::new(&user, &scores, target_pp, rank, each);

    // Creating the embed
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}

#[derive(Copy, Clone)]
enum PpValue {
    Delta(f32),
    Raw(f32),
}

impl PpValue {
    fn parse(input: &str) -> Option<Self> {
        let pp = input.parse().ok()?;

        let this = if input.starts_with('+') {
            Self::Delta(pp)
        } else {
            Self::Raw(pp)
        };

        Some(this)
    }

    fn value(self) -> f32 {
        match self {
            PpValue::Delta(value) => value,
            PpValue::Raw(value) => value,
        }
    }
}
