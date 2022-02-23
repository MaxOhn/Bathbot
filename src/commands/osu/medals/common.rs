use std::{
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, User, Username};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{get_user, UserArgs},
        parse_discord, DoubleResultCow,
    },
    custom_client::{
        groups::{
            BEATMAP_CHALLENGE_PACKS, BEATMAP_PACKS, BEATMAP_SPOTLIGHTS, DEDICATION, HUSH_HUSH,
            MOD_INTRODUCTION, SEASONAL_SPOTLIGHTS, SKILL,
        },
        OsekaiGrouping, OsekaiMedal, Rarity,
    },
    database::OsuData,
    embeds::{EmbedData, MedalsCommonEmbed, MedalsCommonUser},
    error::Error,
    pagination::{MedalsCommonPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, matcher, InteractionExt, MessageBuilder, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

pub(super) async fn _common(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: CommonArgs,
) -> BotResult<()> {
    let CommonArgs {
        name1,
        name2,
        order,
        filter,
    } = args;

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
    let user_args1 = UserArgs::new(name1.as_str(), GameMode::STD);
    let user_fut1 = get_user(&ctx, &user_args1);

    let user_args2 = UserArgs::new(name2.as_str(), GameMode::STD);
    let user_fut2 = get_user(&ctx, &user_args2);

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
    let medals1 = extract_medals(&user1);
    let mut medals2 = extract_medals(&user2);

    let mut medals = Vec::with_capacity(medals_map.len());

    for (medal_id, achieved1) in medals1 {
        match medals_map.remove(&medal_id) {
            Some(medal) => {
                let achieved2 = medals2.remove(&medal_id);

                let entry = MedalEntry {
                    medal,
                    achieved1: Some(achieved1),
                    achieved2,
                };

                medals.push(entry);
            }
            None => warn!("Missing medal id {medal_id} in DB medals"),
        }
    }

    for (medal_id, achieved2) in medals2 {
        match medals_map.remove(&medal_id) {
            Some(medal) => {
                let entry = MedalEntry {
                    medal,
                    achieved1: None,
                    achieved2: Some(achieved2),
                };

                medals.push(entry);
            }
            None => warn!("Missing medal id {medal_id} in DB medals"),
        }
    }

    match filter {
        CommonFilter::None => {}
        CommonFilter::Unique => {
            medals.retain(|entry| entry.achieved1.is_none() || entry.achieved2.is_none())
        }
        CommonFilter::Group(OsekaiGrouping(group)) => {
            medals.retain(|entry| entry.medal.grouping == group)
        }
    }

    match order {
        CommonOrder::DateFirst => {
            medals.sort_unstable_by_key(|entry| match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => a1.min(a2),
                (Some(a1), None) => a1,
                (None, Some(a2)) => a2,
                (None, None) => unreachable!(),
            })
        }
        CommonOrder::DateLast => {
            medals.sort_unstable_by_key(|entry| match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => Reverse(a1.max(a2)),
                (Some(a1), None) => Reverse(a1),
                (None, Some(a2)) => Reverse(a2),
                (None, None) => unreachable!(),
            })
        }
        CommonOrder::Default => medals.sort_unstable_by(|a, b| a.medal.cmp(&b.medal)),
        CommonOrder::Rarity => {
            if !medals.is_empty() {
                match ctx.clients.custom.get_osekai_ranking::<Rarity>().await {
                    Ok(rarities) => {
                        let rarities: HashMap<_, _> = rarities
                            .into_iter()
                            .map(|entry| (entry.medal_id, entry.possession_percent))
                            .collect();

                        medals.sort_unstable_by(|a, b| {
                            let rarity1 = rarities.get(&a.medal.medal_id).copied().unwrap_or(100.0);
                            let rarity2 = rarities.get(&b.medal.medal_id).copied().unwrap_or(100.0);

                            rarity1.partial_cmp(&rarity2).unwrap_or(Ordering::Equal)
                        });
                    }
                    Err(err) => {
                        let _ = data.error(&ctx, OSEKAI_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }
        }
    }

    let mut winner1 = 0;
    let mut winner2 = 0;

    for entry in &medals {
        match (entry.achieved1, entry.achieved2) {
            (Some(a1), Some(a2)) => match a1 < a2 {
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
            warn!("{report:?}");

            None
        }
    };

    let len = medals.len().min(10);
    let user1 = MedalsCommonUser::new(user1.username, winner1);
    let user2 = MedalsCommonUser::new(user2.username, winner2);

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
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub struct MedalEntry {
    pub medal: OsekaiMedal,
    pub achieved1: Option<DateTime<Utc>>,
    pub achieved2: Option<DateTime<Utc>>,
}

fn extract_medals(user: &User) -> HashMap<u32, DateTime<Utc>> {
    match user.medals.as_ref() {
        Some(medals) => medals
            .iter()
            .map(|medal| (medal.medal_id, medal.achieved_at))
            .collect(),
        None => HashMap::new(),
    }
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

enum CommonOrder {
    DateFirst,
    DateLast,
    Default,
    Rarity,
}

impl Default for CommonOrder {
    fn default() -> Self {
        Self::Default
    }
}

enum CommonFilter {
    None,
    Unique,
    Group(OsekaiGrouping<'static>),
}

impl Default for CommonFilter {
    fn default() -> Self {
        Self::None
    }
}

pub(super) struct CommonArgs {
    name1: Option<Username>,
    name2: Username,
    order: CommonOrder,
    filter: CommonFilter,
}

impl CommonArgs {
    const AT_LEAST_ONE: &'static str = "You need to specify at least one osu username. \
        If you're not linked, you must specify two names.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let osu = ctx.psql().get_user_osu(author_id).await?;

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(user_id) => match parse_discord(ctx, user_id).await? {
                    Ok(osu) => osu.into_username(),
                    Err(content) => return Ok(Err(content)),
                },
                None => arg.into(),
            },
            None => return Ok(Err(Self::AT_LEAST_ONE.into())),
        };

        let args = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(user_id) => match parse_discord(ctx, user_id).await? {
                    Ok(osu) => Self {
                        name1: Some(name2),
                        name2: osu.into_username(),
                        order: CommonOrder::default(),
                        filter: CommonFilter::default(),
                    },
                    Err(content) => return Ok(Err(content)),
                },
                None => Self {
                    name1: Some(name2),
                    name2: arg.into(),
                    order: CommonOrder::default(),
                    filter: CommonFilter::default(),
                },
            },
            None => Self {
                name1: osu.map(OsuData::into_username),
                name2,
                order: CommonOrder::default(),
                filter: CommonFilter::default(),
            },
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut name1 = None;
        let mut name2 = None;
        let mut order = None;
        let mut filter = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    "name1" => name1 = Some(value.into()),
                    "name2" => name2 = Some(value.into()),
                    "sort" => match value.as_str() {
                        "date_first" => order = Some(CommonOrder::DateFirst),
                        "date_last" => order = Some(CommonOrder::DateLast),
                        "default" => order = Some(CommonOrder::Default),
                        "rarity" => order = Some(CommonOrder::Rarity),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "filter" => match value.as_str() {
                        "none" => filter = Some(CommonFilter::None),
                        "unique" => filter = Some(CommonFilter::Unique),
                        "Skill" => filter = Some(CommonFilter::Group(OsekaiGrouping(SKILL))),
                        "Dedication" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(DEDICATION)))
                        }
                        "Hush-Hush" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(HUSH_HUSH)))
                        }
                        "Beatmap_Packs" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(BEATMAP_PACKS)))
                        }
                        "Beatmap_Challenge_Packs" => {
                            filter =
                                Some(CommonFilter::Group(OsekaiGrouping(BEATMAP_CHALLENGE_PACKS)))
                        }
                        "Seasonal_Spotlights" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(SEASONAL_SPOTLIGHTS)))
                        }
                        "Beatmap_Spotlights" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(BEATMAP_SPOTLIGHTS)))
                        }
                        "Mod_Introduction" => {
                            filter = Some(CommonFilter::Group(OsekaiGrouping(MOD_INTRODUCTION)))
                        }
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    "discord1" => match parse_discord(ctx, value).await? {
                        Ok(osu) => name1 = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    "discord2" => match parse_discord(ctx, value).await? {
                        Ok(osu) => name2 = Some(osu.into_username()),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
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
                .get_user_osu(command.user_id()?)
                .await?
                .map(OsuData::into_username),
        };

        let order = order.unwrap_or_default();
        let filter = filter.unwrap_or_default();

        Ok(Ok(CommonArgs {
            name1,
            name2,
            order,
            filter,
        }))
    }
}
