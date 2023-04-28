use std::{borrow::Cow, iter, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{
    constants::OSU_API_ISSUE,
    matcher,
    osu::{approx_more_pp, ExtractablePp, PpListUtil},
    MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::user_not_found;
use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, WhatIfEmbed},
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

pub enum WhatIfData {
    NonTop100,
    NoScores {
        count: usize,
        rank: Option<u32>,
    },
    Top100 {
        bonus_pp: f32,
        count: usize,
        new_pp: f32,
        new_pos: usize,
        max_pp: f32,
        rank: Option<u32>,
    },
}

impl WhatIfData {
    pub fn count(&self) -> usize {
        match self {
            WhatIfData::NonTop100 => 0,
            WhatIfData::NoScores { count, .. } => *count,
            WhatIfData::Top100 { count, .. } => *count,
        }
    }
}

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "whatif",
    desc = "Display the impact of a new X pp score for a user"
)]
pub struct WhatIf<'a> {
    #[command(min_value = 0.0, desc = "Specify a pp amount")]
    pp: f32,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 1000,
        desc = "Specify how many times a score should be added, defaults to 1"
    )]
    count: Option<usize>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl<'m> WhatIf<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, &'static str> {
        let mut pp = None;
        let mut name = None;
        let mut discord = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => pp = Some(num),
                Err(_) => match matcher::get_mention_user(arg) {
                    Some(id) => discord = Some(id),
                    None => name = Some(arg.into()),
                },
            }
        }

        Ok(Self {
            pp: pp.ok_or("You must specify a pp value")?,
            mode,
            name,
            count: None,
            discord,
        })
    }
}

#[command]
#[desc("Display the impact of a new X pp score for a user")]
#[help(
    "Calculate the gain in pp if the user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[alias("wi")]
#[group(Osu)]
pub async fn prefix_whatif(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match WhatIf::args(None, args) {
        Ok(args) => whatif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the impact of a new X pp score for a mania user")]
#[help(
    "Calculate the gain in pp if the mania user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[alias("wim")]
#[group(Mania)]
pub async fn prefix_whatifmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match WhatIf::args(Some(GameModeOption::Mania), args) {
        Ok(args) => whatif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the impact of a new X pp score for a taiko user")]
#[help(
    "Calculate the gain in pp if the taiko user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[alias("wit")]
#[group(Taiko)]
pub async fn prefix_whatiftaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match WhatIf::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => whatif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the impact of a new X pp score for a ctb user")]
#[help(
    "Calculate the gain in pp if the ctb user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wic", "whatifcatch")]
#[group(Catch)]
pub async fn prefix_whatifctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match WhatIf::args(Some(GameModeOption::Catch), args) {
        Ok(args) => whatif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_whatif(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = WhatIf::from_interaction(command.input_data())?;

    whatif(ctx, (&mut command).into(), args).await
}

async fn whatif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: WhatIf<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(ctx, orig, args);
    let count = args.count.unwrap_or(1);
    let pp = args.pp;

    if pp < 0.0 {
        return orig.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
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

    let whatif_data = if scores.is_empty() {
        let pp = iter::repeat(pp)
            .zip(0..)
            .take(count)
            .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

        let rank = match ctx.approx().rank(pp, mode).await {
            Ok(rank) => Some(rank),
            Err(err) => {
                warn!(?err, "Failed to get rank pp");

                None
            }
        };

        WhatIfData::NoScores { count, rank }
    } else if pp < scores.last().and_then(|s| s.pp).unwrap_or(0.0) {
        WhatIfData::NonTop100
    } else {
        let mut pps = scores.extract_pp();
        approx_more_pp(&mut pps, 50);
        let actual = pps.accum_weighted();
        let total = user.stats().pp();
        let bonus_pp = (total - actual).max(0.0);

        let idx = pps
            .iter()
            .position(|&pp_| pp_ < pp)
            .unwrap_or(scores.len() - 1);

        pps.extend(iter::repeat(pp).take(count));
        pps.sort_unstable_by(|a, b| b.total_cmp(a));

        let new_pp = pps.accum_weighted();
        let max_pp = pps.first().copied().unwrap_or(0.0);

        let rank = match ctx.approx().rank(new_pp + bonus_pp, mode).await {
            Ok(rank) => Some(rank),
            Err(err) => {
                warn!(?err, "Failed to get rank pp");

                None
            }
        };

        WhatIfData::Top100 {
            bonus_pp,
            count,
            new_pp,
            new_pos: idx + 1,
            max_pp,
            rank,
        }
    };

    // Sending the embed
    let embed = WhatIfEmbed::new(&user, pp, whatif_data);
    let builder = MessageBuilder::new().embed(embed.build());
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
