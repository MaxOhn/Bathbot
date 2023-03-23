use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    sync::Arc,
};

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::{CountryCode, OsuTrackerCountryDetails, OsuTrackerCountryScore};
use bathbot_util::{osu::ModSelection, CowUtils};
use eyre::Result;
use rosu_v2::{
    prelude::{GameMods, Username},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::{commands::CommandOrigin, Context},
    pagination::OsuTrackerCountryTopPagination,
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, Searchable},
        InteractionCommandExt,
    },
};

use super::{HasMods, ModsResult, ScoreOrder};

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
    #[inline]
    fn default() -> Self {
        Self::Pp
    }
}

impl From<CountryTopOrder> for ScoreOrder {
    #[inline]
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

async fn slash_countrytop(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
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

    let user_id = {
        let orig = CommandOrigin::Interaction {
            command: &mut command,
        };

        user_id!(ctx, orig, args)
    };

    let country_code = match args.country.take() {
        Some(country) => match CountryCode::from_name(&country) {
            Some(code) => Some(code),
            None if country.len() == 2 => Some(CountryCode::from(country)),
            None => {
                let lowercase = country.cow_to_ascii_lowercase();

                if matches!(lowercase.as_ref(), "global" | "world") {
                    None
                } else {
                    let content = format!(
                        "Looks like `{country}` is neither a country name nor a country code"
                    );

                    command.error(&ctx, content).await?;

                    return Ok(());
                }
            }
        },
        None => None,
    };

    let details_fut = ctx
        .client()
        .get_osutracker_country_details(country_code.as_deref().map(|code| code.as_str()));

    let name_fut = async {
        match user_id {
            Some(UserId::Id(user_id)) => ctx.osu_user().name(user_id).await,
            Some(UserId::Name(name)) => Ok(Some(name)),
            None => Ok(None),
        }
    };

    let (details_res, name_res) = tokio::join!(details_fut, name_fut);

    let mut details = match details_res {
        Ok(details) => details,
        Err(err) => {
            let content = format!(
                "Either the country code `{code}` is not supported \
                or the osutracker api has an issue.",
                code = country_code
                    .as_deref()
                    .map(|code| code.as_str())
                    .unwrap_or("Global"),
            );

            let _ = command.error(&ctx, content).await;

            return Err(err.wrap_err("failed to get osutracker country details"));
        }
    };

    let name = match name_res {
        Ok(name_opt) => name_opt,
        Err(err) => {
            warn!("{err:?}");

            None
        }
    };

    let mut scores = details.scores.drain(..).zip(1..).collect();
    let details = OsuTrackerCountryDetailsCompact::from(details);

    filter_scores(&mut scores, &args, mods, name.as_deref()).await;

    let content = write_content(&details.country, &args, mods, scores.len(), name);
    let sort = args.sort.unwrap_or_default().into();

    OsuTrackerCountryTopPagination::builder(details, scores, sort)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

async fn filter_scores(
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
            scores.retain(|(score, _)| !score.mods.intersects(mods))
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

    match args.sort.unwrap_or_default() {
        CountryTopOrder::Acc => {
            scores.sort_by(|(a, _), (b, _)| b.acc.partial_cmp(&a.acc).unwrap_or(Ordering::Equal))
        }
        CountryTopOrder::Date => scores.sort_by_key(|(score, _)| Reverse(score.ended_at)),
        CountryTopOrder::Length => {
            scores.sort_by(|(a, _), (b, _)| {
                let a_len = a.seconds_total as f32 / a.mods.clock_rate();
                let b_len = b.seconds_total as f32 / b.mods.clock_rate();

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            });
        }
        CountryTopOrder::Misses => scores.sort_by_key(|(score, _)| score.n_misses),
        CountryTopOrder::Pp => {
            scores.sort_by(|(a, _), (b, _)| b.pp.partial_cmp(&a.pp).unwrap_or(Ordering::Equal))
        }
    }

    if args.reverse == Some(true) {
        scores.reverse();
    }
}

pub struct OsuTrackerCountryDetailsCompact {
    pub country: Box<str>,
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
