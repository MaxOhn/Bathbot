use std::{borrow::Cow, cmp::Ordering, iter, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::RankParam,
    embeds::{EmbedData, WhatIfEmbed},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::OSU_API_ISSUE,
        matcher,
        osu::{approx_more_pp, ExtractablePp, PpListUtil},
        ApplicationCommandExt, ChannelExt,
    },
    BotResult, Context,
};

use super::{get_user_and_scores, ScoreArgs, UserArgs};

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
#[command(name = "whatif")]
/// Display the impact of a new X pp score for a user
pub struct WhatIf<'a> {
    #[command(min_value = 0.0)]
    /// Specify a pp amount
    pp: f32,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 1, max_value = 1000)]
    /// Specify how many times a score should be added, defaults to 1
    count: Option<usize>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

impl<'m> WhatIf<'m> {
    fn args(mode: GameModeOption, args: Args<'m>) -> Result<Self, &'static str> {
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
            mode: Some(mode),
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
pub async fn prefix_whatif(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match WhatIf::args(GameModeOption::Osu, args) {
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
pub async fn prefix_whatifmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match WhatIf::args(GameModeOption::Mania, args) {
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
pub async fn prefix_whatiftaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match WhatIf::args(GameModeOption::Taiko, args) {
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
#[alias("wic")]
#[group(Catch)]
pub async fn prefix_whatifctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match WhatIf::args(GameModeOption::Catch, args) {
        Ok(args) => whatif(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_whatif(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = WhatIf::from_interaction(command.input_data())?;

    whatif(ctx, command.into(), args).await
}

async fn whatif(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: WhatIf<'_>) -> BotResult<()> {
    let (name, mode) = name_mode!(ctx, orig, args);
    let count = args.count.unwrap_or(1);
    let pp = args.pp;

    if pp < 0.0 {
        return orig.error(&ctx, "The pp number must be non-negative").await;
    } else if pp > (i64::MAX / 1024) as f32 {
        return orig.error(&ctx, "Number too large").await;
    }

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100);

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let whatif_data = if scores.is_empty() {
        let pp = iter::repeat(pp)
            .zip(0..)
            .take(count)
            .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

        let rank_result = ctx.client().get_rank_data(mode, RankParam::Pp(pp)).await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                let report = Report::new(why).wrap_err("error while getting rank pp");
                warn!("{report:?}");

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
        let total = user.statistics.as_ref().map_or(0.0, |stats| stats.pp);
        let bonus_pp = total - actual;

        let idx = pps
            .iter()
            .position(|&pp_| pp_ < pp)
            .unwrap_or_else(|| scores.len() - 1);

        pps.extend(iter::repeat(pp).take(count));
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        let new_pp = pps.accum_weighted();
        let max_pp = pps.first().copied().unwrap_or(0.0);

        let rank_fut = ctx
            .client()
            .get_rank_data(mode, RankParam::Pp(new_pp + bonus_pp));

        let rank = match rank_fut.await {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                let report = Report::new(why).wrap_err("error while getting rank pp");
                warn!("{report:?}");

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
    let embed = WhatIfEmbed::new(user, pp, whatif_data).into_builder();
    let builder = MessageBuilder::new().embed(embed.build());
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
