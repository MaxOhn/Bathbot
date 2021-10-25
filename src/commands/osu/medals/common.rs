use crate::{
    custom_client::OsekaiMedal,
    database::OsuData,
    embeds::{EmbedData, MedalsCommonEmbed, MedalsCommonUser},
    pagination::{MedalsCommonPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, matcher, MessageBuilder, MessageExt,
    },
    Args, BotResult, CommandData, Context, Name,
};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, User};
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _common(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: CommonArgs,
) -> BotResult<()> {
    let CommonArgs { name1, name2 } = args;

    let name1 = match name1 {
        Some(name) => name,
        None => {
            let content =
                "Since you're not linked with the `link` command, you must specify two names.";

            return data.error(&ctx, content).await;
        }
    };

    if name1 == name2 {
        return data.error(&ctx, "Give two different names").await;
    }

    // Retrieve all users and their scores
    let user_fut1 = super::request_user(&ctx, &name1, GameMode::STD);
    let user_fut2 = super::request_user(&ctx, &name2, GameMode::STD);
    let medals_fut = ctx.psql().get_medals();

    let (user1, user2, mut medals_map) = match tokio::join!(user_fut1, user_fut2, medals_fut) {
        (Ok(user1), Ok(user2), Ok(medals)) => (user1, user2, medals),
        (Err(why), ..) | (_, Err(why), _) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        (.., Err(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if user1.user_id == user2.user_id {
        let content = "Give two different users";

        return data.error(&ctx, content).await;
    }

    // Combining and sorting all medals
    let medals1 = match extract_medals(&user1) {
        Some(medals) => medals,
        None => {
            let content = format!("`{}` has not achieved any medals :(", user1.username);

            return data.error(&ctx, content).await;
        }
    };

    let medals2 = match extract_medals(&user2) {
        Some(medals) => medals,
        None => {
            let content = format!("`{}` has not achieved any medals :(", user2.username);

            return data.error(&ctx, content).await;
        }
    };
    let mut medals = Vec::with_capacity(medals_map.len());

    for medal_id in medals1.keys() {
        match medals_map.remove(medal_id) {
            Some(medal) => medals.push(medal),
            None => warn!("Missing medal id {} in DB medals", medal_id),
        }
    }

    for medal_id in medals2.keys() {
        if let Some(medal) = medals_map.remove(medal_id) {
            medals.push(medal);
        }
    }

    medals.sort_unstable();

    let mut winner1 = 0;
    let mut winner2 = 0;

    for OsekaiMedal { medal_id, .. } in &medals {
        match (medals1.get(medal_id), medals2.get(medal_id)) {
            (Some(date1), Some(date2)) => match date1 < date2 {
                true => winner1 += 1,
                false => winner2 += 1,
            },
            (Some(_), None) => winner1 += 1,
            (None, Some(_)) => winner2 += 1,
            (None, None) => unreachable!(),
        }
    }

    // Create the thumbnail
    let urls = [user1.avatar_url.as_str(), user2.avatar_url.as_str()];

    let thumbnail = match get_combined_thumbnail(&ctx, urls, 2).await {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to combine avatars");
            warn!("{:?}", report);

            None
        }
    };

    let user1 = MedalsCommonUser::new(user1.username, medals1, winner1);
    let user2 = MedalsCommonUser::new(user2.username, medals2, winner2);
    let len = medals.len().min(10);
    let embed_data = MedalsCommonEmbed::new(&user1, &user2, &medals[..len], 0);

    let embed = embed_data.into_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = thumbnail.as_deref() {
        builder = builder.file("avatar_fuse.png", bytes);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    if medals.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MedalsCommonPagination::new(response, user1, user2, medals);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            let report = Report::new(why).wrap_err("pagination error");
            warn!("{:?}", report);
        }
    });

    Ok(())
}

fn extract_medals(user: &User) -> Option<HashMap<u32, i64>> {
    let medals = user.medals.as_ref()?;

    (!medals.is_empty()).then(|| {
        medals
            .iter()
            .map(|medal| (medal.medal_id, medal.achieved_at.timestamp()))
            .collect()
    })
}

#[command]
#[short_desc("Compare which of the given users achieved medals first")]
#[usage("[username1] [username2]")]
#[example("badewanne3 5joshi")]
#[aliases("medalcommon")]
pub async fn medalscommon(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match CommonArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(common_args)) => {
                    _common(ctx, CommandData::Message { msg, args, num }, common_args).await
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

pub(super) struct CommonArgs {
    name1: Option<Name>,
    name2: Name,
}

const MEDAL_COMMON: &str = "medal common";

impl CommonArgs {
    const AT_LEAST_ONE: &'static str = "You need to specify at least one osu username. \
        If you're not linked, you must specify two names.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let osu = ctx.psql().get_user_osu(author_id).await?;

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.psql().get_user_osu(UserId(id)).await? {
                    Some(osu) => osu.into_username(),
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => arg.into(),
            },
            None => return Ok(Err(Self::AT_LEAST_ONE.into())),
        };

        let args = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.psql().get_user_osu(UserId(id)).await? {
                    Some(osu) => Self {
                        name1: Some(name2),
                        name2: osu.into_username(),
                    },
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => Self {
                    name1: Some(name2),
                    name2: arg.into(),
                },
            },
            None => Self {
                name1: osu.map(OsuData::into_username),
                name2,
            },
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut name1 = None;
        let mut name2 = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name1" => name1 = Some(value.into()),
                    "name2" => name2 = Some(value.into()),
                    "discord1" => match value.parse() {
                        Ok(id) => match ctx.psql().get_user_osu(UserId(id)).await? {
                            Some(osu) => name1 = Some(osu.into_username()),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => bail_cmd_option!("medal common discord1", string, value),
                    },
                    "discord2" => match value.parse() {
                        Ok(id) => match ctx.psql().get_user_osu(UserId(id)).await? {
                            Some(osu) => name2 = Some(osu.into_username()),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => bail_cmd_option!("medal common discord2", string, value),
                    },
                    _ => bail_cmd_option!(MEDAL_COMMON, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(MEDAL_COMMON, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(MEDAL_COMMON, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(MEDAL_COMMON, subcommand, name)
                }
            }
        }

        let (name1, name2) = match (name1, name2) {
            (name1, Some(name)) => (name1, name),
            (Some(name), None) => (None, name),
            (None, None) => return Ok(Err(Self::AT_LEAST_ONE.into())),
        };

        let name1 = match name1 {
            Some(name) => Some(name),
            None => ctx
                .psql()
                .get_user_osu(author_id)
                .await?
                .map(OsuData::into_username),
        };

        Ok(Ok(CommonArgs { name1, name2 }))
    }
}
