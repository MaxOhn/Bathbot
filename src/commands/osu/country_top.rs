use std::{fmt::Write, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{CountryCode as CountryCodeRosu, GameMode, GameMods, OsuError, Username};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use crate::{
    commands::{osu::UserArgs, DoubleResultCow, MyCommand, MyCommandOption},
    core::{commands::CommandData, Context},
    custom_client::{OsuTrackerCountryDetails, OsuTrackerCountryScore},
    database::OsuData,
    embeds::{EmbedData, OsuTrackerCountryTopEmbed},
    error::Error,
    pagination::{OsuTrackerCountryTopPagination, Pagination},
    util::{
        constants::{
            common_literals::{ACC, ACCURACY, COUNTRY, MODS, REVERSE, SORT},
            GENERAL_ISSUE, OSUTRACKER_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::{ModSelection, ScoreOrder},
        ApplicationCommandExt, CountryCode, CowUtils, FilterCriteria, MessageBuilder, MessageExt,
        Searchable,
    },
    BotResult,
};

use super::option_mods_explicit;

async fn countrytop_(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut args: CountryTopArgs,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    let country_code = match args.country.take() {
        Some(code) => code,
        None => match ctx
            .psql()
            .get_user_osu(author_id)
            .await
            .map(|osu| osu.map(OsuData::into_username))
        {
            Ok(Some(name)) => {
                let user_args = UserArgs::new(name.as_str(), GameMode::STD);

                let user = match ctx.redis().osu_user(&user_args).await {
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

    let mut scores = details.scores.drain(..).zip(1..).collect();
    let details = OsuTrackerCountryDetailsCompact::from(details);

    filter_scores(&ctx, &mut scores, &args).await;

    let pages = numbers::div_euclid(10, scores.len());
    let initial = &scores[..scores.len().min(10)];

    let embed = OsuTrackerCountryTopEmbed::new(&details, initial, args.sort_by, (1, pages))
        .into_builder()
        .build();

    let content = write_content(&details.country, &args, scores.len());
    let builder = MessageBuilder::new().embed(embed).content(content);

    let response_raw = data.create_message(&ctx, builder).await?;

    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerCountryTopPagination::new(response, details, scores, args.sort_by);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn filter_scores(
    ctx: &Context,
    scores: &mut Vec<(OsuTrackerCountryScore, usize)>,
    args: &CountryTopArgs,
) {
    match args.mods {
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

struct CountryTopArgs {
    country: Option<CountryCode>,
    mods: Option<ModSelection>,
    sort_by: ScoreOrder,
    reverse: bool,
    query: Option<String>,
    username: Option<Username>,
}

impl CountryTopArgs {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

    pub fn slash(command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let mut country = None;
        let mut mods = None;
        let mut sort_by = None;
        let mut reverse = None;
        let mut query = None;
        let mut username = None;

        for option in command.yoink_options() {
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

                                return Ok(Err(content.into()));
                            };

                            country = Some(country_)
                        }
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    SORT => match value.as_str() {
                        ACC => sort_by = Some(ScoreOrder::Acc),
                        "date" => sort_by = Some(ScoreOrder::Date),
                        "len" => sort_by = Some(ScoreOrder::Length),
                        "miss" => sort_by = Some(ScoreOrder::Misses),
                        "pp" => sort_by = Some(ScoreOrder::Pp),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "query" => query = Some(value),
                    "username" => username = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Boolean(value) => match option.name.as_str() {
                    REVERSE => reverse = Some(value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self {
            country,
            mods,
            sort_by: sort_by.unwrap_or(ScoreOrder::Pp),
            reverse: reverse.unwrap_or(false),
            query,
            username,
        };

        Ok(Ok(args))
    }
}

fn write_content(name: &str, args: &CountryTopArgs, amount: usize) -> String {
    if args.query.is_some() || args.mods.is_some() || args.username.is_some() {
        content_with_condition(name, args, amount)
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

fn content_with_condition(name: &str, args: &CountryTopArgs, amount: usize) -> String {
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

    if let Some(selection) = args.mods {
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

pub async fn slash_countrytop(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match CountryTopArgs::slash(&mut command)? {
        Ok(args) => countrytop_(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_countrytop() -> MyCommand {
    let country =
        MyCommandOption::builder(COUNTRY, "Specify a country (code)").string(Vec::new(), false);

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
        CommandOptionChoice::String {
            name: "misses".to_owned(),
            value: "miss".to_owned(),
        },
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .help("Choose how the scores should be ordered, defaults to `pp`.")
        .string(sort_choices, false);

    let mods = option_mods_explicit();

    let reverse =
        MyCommandOption::builder(REVERSE, "Reverse the resulting score list").boolean(false);

    let query_description = "Search for a specific artist, title, difficulty, or mapper";

    let query = MyCommandOption::builder("query", query_description).string(Vec::new(), false);

    let username = MyCommandOption::builder("username", "Only keep scores from this user")
        .string(Vec::new(), false);

    let options = vec![country, sort, mods, reverse, query, username];

    MyCommand::new("countrytop", "Display the country's top scores").options(options)
}
