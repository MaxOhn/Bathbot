use std::{
    cmp::{Ordering, Reverse},
    mem,
    sync::Arc,
};

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    commands::{
        osu::{get_osekai_medals, get_user, UserArgs},
        parse_discord, DoubleResultCow,
    },
    custom_client::{
        groups::{
            BEATMAP_CHALLENGE_PACKS, BEATMAP_PACKS, BEATMAP_SPOTLIGHTS, DEDICATION, HUSH_HUSH,
            MOD_INTRODUCTION, SEASONAL_SPOTLIGHTS, SKILL,
        },
        OsekaiGrouping, OsekaiMedal, Rarity,
    },
    database::UserConfig,
    embeds::{EmbedData, MedalsListEmbed},
    error::Error,
    pagination::{MedalsListPagination, Pagination},
    util::{
        constants::{
            common_literals::{DISCORD, NAME, REVERSE},
            OSEKAI_ISSUE, OSU_API_ISSUE,
        },
        numbers, InteractionExt, MessageBuilder, MessageExt,
    },
    BotResult, CommandData, Context,
};

pub(super) async fn _medalslist(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: ListArgs,
) -> BotResult<()> {
    let ListArgs {
        config,
        order,
        group,
        reverse,
    } = args;

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);
    let user_fut = get_user(&ctx, &user_args);
    let medals_fut = get_osekai_medals(&ctx);
    let rarity_fut = ctx.clients.custom.get_osekai_ranking::<Rarity>();

    let (mut user, mut osekai_medals, rarities) =
        match tokio::join!(user_fut, medals_fut, rarity_fut) {
            (Ok(user), Ok(medals), Ok(rarities)) => (user, medals, rarities),
            (Err(OsuError::NotFound), ..) => {
                let content = format!("User `{name}` was not found");

                return data.error(&ctx, content).await;
            }
            (Err(err), ..) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
            (_, Err(err), _) | (.., Err(err)) => {
                let _ = data.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.into());
            }
        };

    let rarities: HashMap<_, _> = rarities
        .into_iter()
        .map(|entry| (entry.medal_id, entry.possession_percent))
        .collect();

    let acquired = (
        user.medals.as_ref().map_or(0, Vec::len),
        osekai_medals.len(),
    );

    osekai_medals.sort_unstable_by_key(|medal| medal.medal_id);

    let mut medals = Vec::with_capacity(acquired.0);

    let medals_iter = user
        .medals
        .as_mut()
        .map_or_else(Vec::new, mem::take)
        .into_iter()
        .filter_map(|m| {
            match osekai_medals
                .iter()
                .position(|m_| m_.medal_id == m.medal_id)
            {
                Some(idx) => {
                    let entry = MedalEntryList {
                        medal: osekai_medals.swap_remove(idx),
                        achieved: m.achieved_at,
                        rarity: rarities.get(&m.medal_id).copied().unwrap_or(100.0),
                    };

                    Some(entry)
                }
                None => {
                    warn!("Missing medal id {}", m.medal_id);

                    None
                }
            }
        });

    medals.extend(medals_iter);

    if let Some(OsekaiGrouping(group)) = group {
        medals.retain(|entry| entry.medal.grouping == group);
    }

    let order_str = match order {
        ListOrder::Alphabet => {
            medals.sort_unstable_by(|a, b| a.medal.name.cmp(&b.medal.name));

            "alphabet"
        }
        ListOrder::Date => {
            medals.sort_unstable_by_key(|entry| Reverse(entry.achieved));

            "date"
        }
        ListOrder::MedalId => {
            medals.sort_unstable_by_key(|entry| entry.medal.medal_id);

            "medal id"
        }
        ListOrder::Rarity => {
            medals.sort_unstable_by(|a, b| {
                a.rarity.partial_cmp(&b.rarity).unwrap_or(Ordering::Equal)
            });

            "rarity"
        }
    };

    let reverse_str = if reverse {
        medals.reverse();

        "reversed "
    } else {
        ""
    };

    let len = medals.len().min(10);
    let pages = numbers::div_euclid(10, medals.len());
    let embed_data = MedalsListEmbed::new(&user, &medals[..len], acquired, (1, pages));

    let content = match group {
        None => format!("All medals of `{name}` sorted by {reverse_str}{order_str}:"),
        Some(OsekaiGrouping(group)) => {
            format!("All `{group}` medals of `{name}` sorted by {reverse_str}{order_str}:")
        }
    };

    let builder = MessageBuilder::new()
        .embed(embed_data.into_builder())
        .content(content);

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if medals.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MedalsListPagination::new(response, user, medals, acquired);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub struct MedalEntryList {
    pub medal: OsekaiMedal,
    pub achieved: DateTime<Utc>,
    pub rarity: f32,
}

enum ListOrder {
    Alphabet,
    Date,
    MedalId,
    Rarity,
}

impl Default for ListOrder {
    fn default() -> Self {
        Self::Date
    }
}

pub struct ListArgs {
    config: UserConfig,
    order: ListOrder,
    reverse: bool,
    group: Option<OsekaiGrouping<'static>>,
}

impl ListArgs {
    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut order = None;
        let mut group = None;
        let mut reverse = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    "sort" => match value.as_str() {
                        "alphabet" => order = Some(ListOrder::Alphabet),
                        "date" => order = Some(ListOrder::Date),
                        "medal_id" => order = Some(ListOrder::MedalId),
                        "rarity" => order = Some(ListOrder::Rarity),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "group" => match value.as_str() {
                        "Skill" => group = Some(OsekaiGrouping(SKILL)),
                        "Dedication" => group = Some(OsekaiGrouping(DEDICATION)),
                        "Hush-Hush" => group = Some(OsekaiGrouping(HUSH_HUSH)),
                        "Beatmap_Packs" => group = Some(OsekaiGrouping(BEATMAP_PACKS)),
                        "Beatmap_Challenge_Packs" => {
                            group = Some(OsekaiGrouping(BEATMAP_CHALLENGE_PACKS))
                        }
                        "Seasonal_Spotlights" => group = Some(OsekaiGrouping(SEASONAL_SPOTLIGHTS)),
                        "Beatmap_Spotlights" => group = Some(OsekaiGrouping(BEATMAP_SPOTLIGHTS)),
                        "Mod_Introduction" => group = Some(OsekaiGrouping(MOD_INTRODUCTION)),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Boolean(value) => match option.name.as_str() {
                    REVERSE => reverse = Some(value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        Ok(Ok(Self {
            config,
            order: order.unwrap_or_default(),
            group,
            reverse: reverse.unwrap_or(false),
        }))
    }
}
