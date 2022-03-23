use std::{cmp::Reverse, sync::Arc};

use chrono::{DateTime, Utc};
use eyre::Report;
use rosu_v2::prelude::{GameMode, MedalCompact, OsuError, User, Username};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
        parse_discord, DoubleResultCow,
    },
    database::OsuData,
    embeds::MedalEmbed,
    error::Error,
    pagination::MedalRecentPagination,
    util::{
        constants::{
            common_literals::{DISCORD, INDEX, NAME},
            GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE,
        },
        InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
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

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);
    let user_fut = get_user(&ctx, &user_args);
    let redis = ctx.redis();

    let (mut user, mut all_medals) = match tokio::join!(user_fut, redis.medals()) {
        (Ok(user), Ok(medals)) => (user, medals.to_inner()),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        (Err(err), _) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
        (_, Err(err)) => {
            let _ = data.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
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

    let (medal_id, achieved_at) = match achieved_medals.get(index - 1) {
        Some(MedalCompact {
            medal_id,
            achieved_at,
        }) => (*medal_id, *achieved_at),
        None => {
            let content = format!(
                "`{}` only has {} medals, cannot show medal #{index}",
                user.username,
                achieved_medals.len(),
            );

            return data.error(&ctx, content).await;
        }
    };

    let medal = match all_medals.iter().position(|m| m.medal_id == medal_id) {
        Some(idx) => all_medals.swap_remove(idx),
        None => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            bail!("No medal with id `{medal_id}`");
        }
    };

    let content = match index % 10 {
        1 if index == 1 => "Most recent medal:".to_owned(),
        1 if index != 11 => format!("{index}st most recent medal:"),
        2 if index != 12 => format!("{index}nd most recent medal:"),
        3 if index != 13 => format!("{index}rd most recent medal:"),
        _ => format!("{index}th most recent medal:"),
    };

    let achieved = MedalAchieved {
        user: &user,
        achieved_at,
        index,
        medal_count: achieved_medals.len(),
    };

    let embed_data = MedalEmbed::new(medal.clone(), Some(achieved), Vec::new(), None);
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
        all_medals,
    );

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(why));
        }
    });

    Ok(())
}

pub(super) struct RecentArgs {
    pub name: Option<Username>,
    pub index: Option<usize>,
}

impl RecentArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
        index: Option<usize>,
    ) -> DoubleResultCow<Self> {
        let name = match args.next() {
            Some(arg) => match check_user_mention(ctx, arg).await? {
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
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut username = None;
        let mut index = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => username = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => {
                    let number = (option.name == INDEX)
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

                    index = Some(number.max(1) as usize)
                }
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => username = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let name = match username {
            Some(name) => Some(name),
            None => ctx
                .psql()
                .get_user_osu(command.user_id()?)
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
