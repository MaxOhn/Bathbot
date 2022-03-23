mod query;
mod user;

use std::{cmp::Reverse, str::FromStr, sync::Arc};

use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use crate::{
    commands::{parse_discord, DoubleResultCow, MyCommand, MyCommandOption},
    core::Context,
    custom_client::OsekaiBadge,
    database::UserConfig,
    error::Error,
    util::{
        constants::common_literals::{DISCORD, NAME, SORT},
        InteractionExt, MessageExt,
    },
    BotResult,
};

pub use query::handle_autocomplete as handle_badge_autocomplete;

use super::{option_discord, option_name, require_link};

struct BadgeArgs {
    kind: BadgeCommandKind,
    sort_by: BadgeOrder,
}

enum BadgeOrder {
    Alphabet,
    Date,
    OwnerCount,
}

impl BadgeOrder {
    fn apply(self, badges: &mut [OsekaiBadge]) {
        match self {
            Self::Alphabet => badges.sort_unstable_by(|a, b| a.name.cmp(&b.name)),
            Self::Date => badges.sort_unstable_by_key(|badge| Reverse(badge.awarded_at)),
            Self::OwnerCount => badges.sort_unstable_by_key(|badge| Reverse(badge.users.len())),
        }
    }
}

impl FromStr for BadgeOrder {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alphabet" => Ok(BadgeOrder::Alphabet),
            "date" => Ok(BadgeOrder::Date),
            "owner_count" => Ok(BadgeOrder::OwnerCount),
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

impl Default for BadgeOrder {
    fn default() -> Self {
        Self::Date
    }
}

enum BadgeCommandKind {
    Query { name: String },
    User { config: UserConfig },
}

impl BadgeArgs {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        let mut kind = None;
        let mut sort_by = None;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "query" => {
                    for option in options {
                        match option.value {
                            CommandOptionValue::String(name) => match option.name.as_str() {
                                NAME => kind = Some(BadgeCommandKind::Query { name }),
                                SORT => sort_by = Some(name.parse()?),
                                _ => return Err(Error::InvalidCommandOptions),
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        }
                    }
                }
                "user" => {
                    let mut config = ctx.user_config(command.user_id()?).await?;

                    for option in options {
                        match option.value {
                            CommandOptionValue::String(value) => match option.name.as_str() {
                                NAME => config.osu = Some(value.into()),
                                SORT => sort_by = Some(value.parse()?),
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

                    kind = Some(BadgeCommandKind::User { config });
                }
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }

        let args = Self {
            kind: kind.ok_or(Error::InvalidCommandOptions)?,
            sort_by: sort_by.unwrap_or_default(),
        };

        Ok(Ok(args))
    }
}

pub async fn slash_badges(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let BadgeArgs { kind, sort_by } = match BadgeArgs::slash(&ctx, &mut command).await? {
        Ok(args) => args,
        Err(content) => return command.error(&ctx, content).await,
    };

    match kind {
        BadgeCommandKind::Query { name } => query::query_(ctx, command, name, sort_by).await,
        BadgeCommandKind::User { config } => {
            user::user_(ctx, command.into(), config, sort_by).await
        }
    }
}

fn sort_option() -> MyCommandOption {
    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "Alphabet".to_owned(),
            value: "alphabet".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Owner count".to_owned(),
            value: "owner_count".to_owned(),
        },
    ];

    let sort_description = "Choose how the badges should be ordered, defaults to date";

    MyCommandOption::builder(SORT, sort_description).string(sort_choices, false)
}

pub fn define_badges() -> MyCommand {
    let name = MyCommandOption::builder(NAME, "Specify the badge name or acronym")
        .autocomplete()
        .string(Vec::new(), true);

    let query = MyCommandOption::builder("query", "Display all badges matching the query")
        .subcommand(vec![name, sort_option()]);

    let name = option_name();
    let discord = option_discord();
    let options = vec![name, sort_option(), discord];

    let user = MyCommandOption::builder("user", "Display all badges of a user").subcommand(options);

    MyCommand::new("badges", "Display info about badges").options(vec![query, user])
}
