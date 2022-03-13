use std::{mem, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{CountryCode as CountryCodeRosu, GameMode, OsuError};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{
        osu::{get_user_cached, UserArgs},
        MyCommand, MyCommandOption,
    },
    core::{commands::CommandData, Context},
    custom_client::OsuTrackerCountryDetails,
    database::OsuData,
    embeds::{EmbedData, OsuTrackerCountryTopEmbed},
    error::Error,
    pagination::{OsuTrackerCountryTopPagination, Pagination},
    util::{
        constants::{common_literals::COUNTRY, GENERAL_ISSUE, OSUTRACKER_ISSUE, OSU_API_ISSUE},
        numbers, ApplicationCommandExt, CountryCode, MessageExt,
    },
    BotResult,
};

async fn countrytop_(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    country_code: Option<CountryCode>,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    let country_code = match country_code {
        Some(code) => code,
        None => match ctx
            .psql()
            .get_user_osu(author_id)
            .await
            .map(|osu| osu.map(OsuData::into_username))
        {
            Ok(Some(name)) => {
                let user_args = UserArgs::new(name.as_str(), GameMode::STD);

                let user = match get_user_cached(&ctx, &user_args).await {
                    Ok(user) => user,
                    Err(OsuError::NotFound) => {
                        let content = format!("User `{name}` was not found");

                        return data.error(&ctx, content).await;
                    }
                    Err(err) => {
                        let _ = data.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                };

                user.country_code.as_str().into()
            }
            Ok(None) => {
                let content = "Since you're not linked, you must specify a country (code)";

                return data.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let details_fut = ctx
        .clients
        .custom
        .get_osutracker_country_details(country_code.as_str());

    let mut details = match details_fut.await {
        Ok(details) => details,
        Err(err) => {
            let _ = data.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    let scores = mem::take(&mut details.scores);
    let details = OsuTrackerCountryDetailsCompact::from(details);

    // TODO: Check global

    let pages = numbers::div_euclid(10, scores.len());
    let initial = &scores[..scores.len().min(10)];

    let embed = OsuTrackerCountryTopEmbed::new(&details, initial, (1, pages))
        .into_builder()
        .build();

    let response_raw = data.create_message(&ctx, embed.into()).await?;

    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerCountryTopPagination::new(response, details, scores);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub struct OsuTrackerCountryDetailsCompact {
    pub country: String,
    pub code: CountryCodeRosu,
    pub pp: f32,
}

impl From<OsuTrackerCountryDetails> for OsuTrackerCountryDetailsCompact {
    fn from(details: OsuTrackerCountryDetails) -> Self {
        Self {
            country: details.country,
            code: details.code,
            pp: details.pp,
        }
    }
}

pub async fn slash_countrytop(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut country = None;

    if let Some(option) = command.yoink_options().pop() {
        match option.value {
            CommandOptionValue::String(mut value) => match option.name.as_str() {
                COUNTRY => match value.as_str() {
                    "global" | "world" => country = Some("global".into()),
                    _ => {
                        let country_ = if value.len() == 2 && value.is_ascii() {
                            value.make_ascii_uppercase();

                            value.into()
                        } else if let Some(code) = CountryCode::from_name(&value) {
                            code
                        } else {
                            let content = format!(
                                "Failed to parse `{value}` as country or country code.\n\
                                Be sure to specify a valid country or two ASCII letter country code."
                            );

                            return command.error(&ctx, content).await;
                        };

                        country = Some(country_)
                    }
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    countrytop_(ctx, command.into(), country).await
}

pub fn define_countrytop() -> MyCommand {
    let country =
        MyCommandOption::builder(COUNTRY, "Specify a country (code)").string(Vec::new(), false);

    MyCommand::new("countrytop", "Display the country's top scores").options(vec![country])
}
