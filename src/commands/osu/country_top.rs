use std::{fmt::Write, sync::Arc};

use command_macros::{HasMods, HasName, SlashCommand};
use eyre::Report;
use rosu_v2::{
    prelude::{GameMode, GameMods, OsuError, Username},
    OsuResult,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    core::Context,
    custom_client::{OsuTrackerCountryDetails, OsuTrackerCountryScore},
    database::OsuData,
    embeds::{EmbedData, OsuTrackerCountryTopEmbed},
    pagination::{OsuTrackerCountryTopPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers,
        osu::ModSelection,
        query::{FilterCriteria, Searchable},
        ApplicationCommandExt, Authored, CountryCode, CowUtils,
    },
    BotResult,
};

use super::{
    HasMods, HasName, ModsResult, ScoreOrder, UserArgs, UsernameFutureResult, UsernameResult,
};

#[derive(CommandModel, CreateCommand, HasMods, HasName, SlashCommand)]
#[command(name = "countrytop")]
/// Display the country's top scores
pub struct CountryTop {
    /// Specify a country (code)
    country: Option<String>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<CountryTopOrder>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<String>,
    /// Reverse the resulting score list
    reverse: Option<bool>,
    /// Search for a specific artist, title, difficulty, or mapper
    query: Option<String>,
    /// Only keep scores from this username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Only keep scores from this discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum CountryTopOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
}

impl Default for CountryTopOrder {
    fn default() -> Self {
        Self::Pp
    }
}

impl From<CountryTopOrder> for ScoreOrder {
    fn from(sort: CountryTopOrder) -> Self {
        match sort {
            CountryTopOrder::Acc => Self::Acc,
            CountryTopOrder::Date => Self::Date,
            CountryTopOrder::Length => Self::Length,
            CountryTopOrder::Misses => Self::Misses,
            CountryTopOrder::Pp => Self::Pp,
        }
    }
}

async fn slash_countrytop(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let mut args = CountryTop::from_interaction(command.input_data())?;

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let name = match args.username(&ctx) {
        UsernameResult::Name(name) => Some(name),
        UsernameResult::None => None,
        UsernameResult::Future(fut) => match fut.await {
            UsernameFutureResult::Name(name) => Some(name),
            UsernameFutureResult::NotLinked(user_id) => {
                let content = format!("<@{user_id}> is not linked to an osu!profile");
                command.error(&ctx, content).await?;

                return Ok(());
            }
            UsernameFutureResult::Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let author = command.user_id()?;

    let country_code = if let Some(ref name) = name {
        match user_country(&ctx, name).await {
            Ok(code) => code,
            Err(OsuError::NotFound) => {
                let content = format!("User `{name}` was not found");
                command.error(&ctx, content).await?;

                return Ok(());
            }
            Err(err) => {
                let _ = command.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        match args.country.take() {
            Some(country) => match CountryCode::from_name(&country) {
                Some(code) => code,
                None => {
                    if country.len() == 2 {
                        CountryCode::from(country)
                    } else {
                        let content = format!(
                            "Looks like `{country}` is neither a country name nor a country code"
                        );

                        command.error(&ctx, content).await?;

                        return Ok(());
                    }
                }
            },
            None => match ctx
                .psql()
                .get_user_osu(author)
                .await
                .map(|osu| osu.map(OsuData::into_username))
            {
                Ok(Some(name)) => match user_country(&ctx, &name).await {
                    Ok(code) => code,
                    Err(OsuError::NotFound) => {
                        let content = format!("User `{name}` was not found");
                        command.error(&ctx, content).await?;

                        return Ok(());
                    }
                    Err(err) => {
                        let _ = command.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                },
                Ok(None) => {
                    let content = "Since you're not linked, you must specify a country (code)";
                    command.error(&ctx, content).await?;

                    return Ok(());
                }
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            },
        }
    };

    let details_fut = ctx
        .client()
        .get_osutracker_country_details(country_code.as_str());

    let mut details = match details_fut.await {
        Ok(details) => details,
        Err(err) => {
            let content = format!(
                "Either the country code `{country_code}` is not supported \
                or the osutracker api has an issue."
            );

            let _ = command.error(&ctx, content).await;

            return Err(err.into());
        }
    };

    let mut scores = details.scores.drain(..).zip(1..).collect();
    let details = OsuTrackerCountryDetailsCompact::from(details);

    filter_scores(&ctx, &mut scores, &args, mods, name.as_deref()).await;

    let pages = numbers::div_euclid(10, scores.len());
    let initial = &scores[..scores.len().min(10)];
    let sort = args.sort.unwrap_or_default().into();

    let embed = OsuTrackerCountryTopEmbed::new(&details, initial, sort, (1, pages))
        .build();

    let content = write_content(&details.country, &args, mods, scores.len(), name);
    let builder = MessageBuilder::new().embed(embed).content(content);

    let response_raw = command.update(&ctx, &builder).await?;

    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;
    let sort = args.sort.unwrap_or_default().into();

    let pagination = OsuTrackerCountryTopPagination::new(response, details, scores, sort);

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, author, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn filter_scores(
    ctx: &Context,
    scores: &mut Vec<(OsuTrackerCountryScore, usize)>,
    args: &CountryTop,
    mods: Option<ModSelection>,
    name: Option<&str>,
) {
    match mods {
        Some(ModSelection::Include(GameMods::NoMod)) => {
            scores.retain(|(score, _)| score.mods.is_empty())
        }
        Some(ModSelection::Include(mods)) => {
            scores.retain(|(score, _)| score.mods.intersection(mods) == mods)
        }
        Some(ModSelection::Exact(mods)) => scores.retain(|(score, _)| score.mods == mods),
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores.retain(|(score, _)| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|(score, _)| score.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);

        scores.retain(|(score, _)| score.matches(&criteria));
    }

    if let Some(username) = name {
        let username = username.cow_to_ascii_lowercase();

        scores.retain(|(score, _)| score.player.cow_to_ascii_lowercase() == username);
    }

    let sort = args.sort.unwrap_or_default();
    ScoreOrder::from(sort).apply(ctx, scores).await;

    if args.reverse == Some(true) {
        scores.reverse();
    }
}

pub struct OsuTrackerCountryDetailsCompact {
    pub country: String,
    pub code: rosu_v2::prelude::CountryCode,
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

fn write_content(
    name: &str,
    args: &CountryTop,
    mods: Option<ModSelection>,
    amount: usize,
    username: Option<Username>,
) -> String {
    if args.query.is_some() || mods.is_some() || username.is_some() {
        content_with_condition(name, args, mods, amount, username)
    } else {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let reverse = if args.reverse == Some(true) {
            "reversed "
        } else {
            ""
        };

        match args.sort.unwrap_or_default() {
            CountryTopOrder::Acc => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}accuracy:")
            }
            CountryTopOrder::Date if args.reverse == Some(true) => {
                format!("Oldest scores in `{name}`'{genitive} top100:")
            }
            CountryTopOrder::Date => format!("Most recent scores in `{name}`'{genitive} top100:"),
            CountryTopOrder::Length => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}length:")
            }
            CountryTopOrder::Misses => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}miss count:")
            }
            CountryTopOrder::Pp => format!("`{name}`'{genitive} top100 sorted by {reverse}pp:"),
        }
    }
}

fn content_with_condition(
    name: &str,
    args: &CountryTop,
    mods: Option<ModSelection>,
    amount: usize,
    username: Option<Username>,
) -> String {
    let mut content = String::with_capacity(64);

    let genitive = if name.ends_with('s') { "" } else { "s" };
    let _ = write!(content, "`{name}`'{genitive} top100  ~ ");

    match args.sort.unwrap_or_default() {
        CountryTopOrder::Acc => content.push_str("`Order: Accuracy"),
        CountryTopOrder::Date => content.push_str("`Order: Date"),
        CountryTopOrder::Length => content.push_str("`Order: Length"),
        CountryTopOrder::Misses => content.push_str("`Order: Miss count"),
        CountryTopOrder::Pp => content.push_str("`Order: Pp"),
    }

    if args.reverse == Some(true) {
        content.push_str(" (reverse)`");
    } else {
        content.push('`');
    }

    if let Some(selection) = mods {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let (pre, mods) = match selection {
            ModSelection::Include(mods) => ("Include ", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Exact(mods) => ("", mods),
        };

        let _ = write!(content, "`Mods: {pre}{mods}`");
    }

    if let Some(query) = args.query.as_deref() {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Query: {query}`");
    }

    if let Some(username) = username.as_deref() {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Username: {username}`");
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching top score{plural}:");

    content
}

async fn user_country(ctx: &Context, name: &str) -> OsuResult<CountryCode> {
    let user_args = UserArgs::new(name, GameMode::STD);
    let user = ctx.redis().osu_user(&user_args).await?;

    Ok(user.country_code.into())
}
