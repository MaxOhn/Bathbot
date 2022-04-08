use std::{fmt::Write, sync::Arc};

use command_macros::{HasName, SlashCommand};
use eyre::Report;
use rosu_v2::prelude::{CountryCode, GameMode, GameMods, OsuError};
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
        constants::{GENERAL_ISSUE, OSUTRACKER_ISSUE, OSU_API_ISSUE},
        matcher, numbers,
        osu::{ModSelection, ScoreOrder},
        query::FilterCriteria,
        ApplicationCommandExt, Authored, CowUtils,
    },
    BotResult,
};

use super::UserArgs;

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
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

#[derive(CommandOption, CreateOption)]
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

async fn slash_countrytop(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let mut args = CountryTop::from_interaction(command.input_data())?;

    let mods = match args.mods.map(|mods| matcher::get_mods(&mods)) {
        Some(mods) => mods,
        None => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return command.error(&ctx, content).await;
        }
    };

    let author = command.user_id()?;

    let country_code = match args.country.take() {
        Some(code) => code,
        None => match ctx
            .psql()
            .get_user_osu(author)
            .await
            .map(|osu| osu.map(OsuData::into_username))
        {
            Ok(Some(name)) => {
                let user_args = UserArgs::new(name.as_str(), GameMode::STD);

                let user = match ctx.redis().osu_user(&user_args).await {
                    Ok(user) => user,
                    Err(OsuError::NotFound) => {
                        let content = format!("User `{name}` was not found");

                        return command.error(&ctx, content).await;
                    }
                    Err(err) => {
                        let _ = command.error(&ctx, OSU_API_ISSUE).await;

                        return Err(err.into());
                    }
                };

                user.country_code.into()
            }
            Ok(None) => {
                let content = "Since you're not linked, you must specify a country (code)";

                return command.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

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
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    let mut scores = details.scores.drain(..).zip(1..).collect();
    let details = OsuTrackerCountryDetailsCompact::from(details);

    filter_scores(&ctx, &mut scores, &args, mods).await;

    let pages = numbers::div_euclid(10, scores.len());
    let initial = &scores[..scores.len().min(10)];

    let embed = OsuTrackerCountryTopEmbed::new(&details, initial, args.sort_by, (1, pages))
        .into_builder()
        .build();

    let content = write_content(&details.country, &args, mods, scores.len());
    let builder = MessageBuilder::new().embed(embed).content(content);

    let response_raw = command.update(&ctx, &builder).await?;

    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerCountryTopPagination::new(response, details, scores, args.sort_by);

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

    if let Some(username) = args.username.as_deref() {
        let username = username.cow_to_ascii_lowercase();

        scores.retain(|(score, _)| score.player.cow_to_ascii_lowercase() == username);
    }

    args.sort_by.apply(ctx, scores).await;

    if args.reverse {
        scores.reverse();
    }
}

pub struct OsuTrackerCountryDetailsCompact {
    pub country: String,
    pub code: CountryCode,
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
) -> String {
    if args.query.is_some() || mods.is_some() || args.username.is_some() {
        content_with_condition(name, args, mods, amount)
    } else {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let reverse = if args.reverse { "reversed " } else { "" };

        match args.sort_by {
            ScoreOrder::Acc => format!("`{name}`'{genitive} top100 sorted by {reverse}accuracy:"),
            ScoreOrder::Date if args.reverse => {
                format!("Oldest scores in `{name}`'{genitive} top100:")
            }
            ScoreOrder::Date => format!("Most recent scores in `{name}`'{genitive} top100:"),
            ScoreOrder::Length => format!("`{name}`'{genitive} top100 sorted by {reverse}length:"),
            ScoreOrder::Misses => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}miss count:")
            }
            ScoreOrder::Pp => format!("`{name}`'{genitive} top100 sorted by {reverse}pp:"),
            _ => unreachable!(),
        }
    }
}

fn content_with_condition(
    name: &str,
    args: &CountryTop,
    mods: Option<ModSelection>,
    amount: usize,
) -> String {
    let mut content = String::with_capacity(64);

    let genitive = if name.ends_with('s') { "" } else { "s" };
    let _ = write!(content, "`{name}`'{genitive} top100  ~ ");

    match args.sort_by {
        ScoreOrder::Acc => content.push_str("`Order: Accuracy"),
        ScoreOrder::Date => content.push_str("`Order: Date"),
        ScoreOrder::Length => content.push_str("`Order: Length"),
        ScoreOrder::Misses => content.push_str("`Order: Miss count"),
        ScoreOrder::Pp => content.push_str("`Order: Pp"),
        _ => unreachable!(),
    }

    if args.reverse {
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

    if let Some(username) = args.username.as_deref() {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Username: {username}`");
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching top score{plural}:");

    content
}
