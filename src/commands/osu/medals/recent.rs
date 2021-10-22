use crate::{
    bail,
    database::OsuData,
    embeds::MedalEmbed,
    pagination::MedalRecentPagination,
    util::{
        constants::{
            common_literals::{DISCORD, INDEX, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use chrono::{DateTime, Utc};
use rosu_v2::prelude::{GameMode, OsuError, User};
use std::{cmp::Reverse, sync::Arc};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

#[command]
#[short_desc("Display a recently acquired medal of a user")]
#[long_desc(
    "Display a recently acquired medal of a user.\n\
    To start from a certain index, specify a number right after the command, e.g. `mr3`."
)]
#[usage("[username]")]
#[example("badewanne3", r#""im a fancy lad""#)]
#[aliases("mr", "recentmedal")]
async fn medalrecent(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(recent_args)) => {
                    _medalrecent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_medal(ctx, *command).await,
    }
}

pub(super) async fn _medalrecent(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentArgs,
) -> BotResult<()> {
    let RecentArgs { name, index } = args;

    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_fut = super::request_user(&ctx, &name, GameMode::STD);

    let mut user = match user_fut.await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let mut achieved_medals = user.medals.take().unwrap_or_default();

    if achieved_medals.is_empty() {
        let content = format!("`{}` has not achieved any medals yet :(", user.username);
        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        return Ok(());
    }

    achieved_medals.sort_unstable_by_key(|medal| Reverse(medal.achieved_at));
    let index = index.unwrap_or(1);

    let (medal, achieved_at) = match achieved_medals.get(index - 1) {
        Some(achieved) => match ctx.psql().get_medal_by_id(achieved.medal_id).await {
            Ok(Some(medal)) => (medal, achieved.achieved_at),
            Ok(None) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                bail!("No medal with id `{}` in DB", achieved.medal_id);
            }
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        },
        None => {
            let content = format!(
                "`{}` only has {} medals, cannot show medal #{}",
                user.username,
                achieved_medals.len(),
                index
            );

            return data.error(&ctx, content).await;
        }
    };

    let content = match index % 10 {
        1 if index == 1 => "Most recent medal:".to_owned(),
        1 if index != 11 => format!("{}st most recent medal:", index),
        2 if index != 12 => format!("{}nd most recent medal:", index),
        3 if index != 13 => format!("{}rd most recent medal:", index),
        _ => format!("{}th most recent medal:", index),
    };

    let achieved = MedalAchieved {
        user: &user,
        achieved_at,
        index,
        medal_count: achieved_medals.len(),
    };

    let embed_data = MedalEmbed::new(medal.clone(), Some(achieved), Vec::new(), Vec::new());
    let embed = embed_data.clone().minimized().build();
    let builder = MessageBuilder::new().embed(embed).content(content);
    let response = data.create_message(&ctx, builder).await?.model().await?;
    let owner = data.author()?.id;

    let pagination = MedalRecentPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        medal,
        achieved_medals,
        index,
        embed_data,
    );

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (medalrecent): {}")
        }
    });

    Ok(())
}

pub(super) struct RecentArgs {
    pub name: Option<Name>,
    pub index: Option<usize>,
}

const MEDAL_RECENT: &str = "medal recent";

impl RecentArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        index: Option<usize>,
    ) -> BotResult<Result<Self, &'static str>> {
        let name = match args.next() {
            Some(arg) => match Args::check_user_mention(ctx, arg).await? {
                Ok(osu) => Some(osu.into_username()),
                Err(content) => return Ok(Err(content)),
            },
            None => ctx
                .psql()
                .get_user_osu(author_id)
                .await?
                .map(OsuData::into_username),
        };

        Ok(Ok(Self { name, index }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut index = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => username = Some(value.into()),
                    DISCORD => username = Some(parse_discord_option!(ctx, value, "medal recent")),
                    _ => bail_cmd_option!(MEDAL_RECENT, string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    INDEX => index = Some(value.max(1) as usize),
                    _ => bail_cmd_option!(MEDAL_RECENT, integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(MEDAL_RECENT, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(MEDAL_RECENT, subcommand, name)
                }
            }
        }

        let name = match username {
            Some(osu) => Some(osu.into_username()),
            None => ctx
                .psql()
                .get_user_osu(author_id)
                .await?
                .map(OsuData::into_username),
        };

        Ok(Ok(RecentArgs { name, index }))
    }
}

pub struct MedalAchieved<'u> {
    pub user: &'u User,
    pub achieved_at: DateTime<Utc>,
    pub index: usize,
    pub medal_count: usize,
}
