use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{
    constants::{OSUSTATS_API_ISSUE, OSU_API_ISSUE},
    matcher, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::OsuStatsCount;
use crate::{
    commands::{osu::user_not_found, GameModeOption},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, OsuStatsCountsEmbed},
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, osu::TopCounts, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "osc",
    desc = "Count how often a user appears on top of map leaderboards"
)]
pub struct Osc<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl<'m> From<Osc<'m>> for OsuStatsCount<'m> {
    fn from(args: Osc<'m>) -> Self {
        Self {
            mode: args.mode,
            name: args.name,
            discord: args.discord,
        }
    }
}

impl<'m> OsuStatsCount<'m> {
    fn args(mode: Option<GameModeOption>, mut args: Args<'m>) -> Self {
        match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => Self {
                    mode,
                    discord: Some(id),
                    name: None,
                },
                None => Self {
                    mode,
                    name: Some(arg.into()),
                    discord: None,
                },
            },
            None => Self {
                mode,
                ..Default::default()
            },
        }
    }
}

#[command]
#[desc("Count how often a user appears on top of a map's leaderboard")]
#[help(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `osu` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osc", "osustatscounts")]
#[group(Osu)]
async fn prefix_osustatscount(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = OsuStatsCount::args(None, args);

    count(ctx, msg.into(), args).await
}

#[command]
#[desc("Count how often a user appears on top of a mania map's leaderboard")]
#[help(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `mania` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscm", "osustatscountsmania")]
#[group(Mania)]
async fn prefix_osustatscountmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = OsuStatsCount::args(Some(GameModeOption::Mania), args);

    count(ctx, msg.into(), args).await
}

#[command]
#[desc("Count how often a user appears on top of a taiko map's leaderboard")]
#[help(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `taiko` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osct", "osustatscountstaiko")]
#[group(Taiko)]
async fn prefix_osustatscounttaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = OsuStatsCount::args(Some(GameModeOption::Taiko), args);

    count(ctx, msg.into(), args).await
}

#[command]
#[desc("Count how often a user appears on top of a ctb map's leaderboard")]
#[help(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `ctb` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscc", "osustatscountsctb", "osustatscountcatch")]
#[group(Catch)]
async fn prefix_osustatscountctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = OsuStatsCount::args(Some(GameModeOption::Catch), args);

    count(ctx, msg.into(), args).await
}

async fn slash_osc(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Osc::from_interaction(command.input_data())?;

    count(ctx, (&mut command).into(), args.into()).await
}

pub(super) async fn count(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: OsuStatsCount<'_>,
) -> Result<()> {
    let (user_id, mode) = user_id_mode!(ctx, orig, args);
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let counts = match TopCounts::request(&ctx, &user, mode).await {
        Ok(counts) => counts,
        Err(err) => {
            let _ = orig.error(&ctx, OSUSTATS_API_ISSUE).await;

            return Err(err.wrap_err("failed to get top counts"));
        }
    };

    let embed_data = OsuStatsCountsEmbed::new(&user, mode, counts);
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}
