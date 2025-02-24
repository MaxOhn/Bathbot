use bathbot_macros::{SlashCommand, command};
use bathbot_util::MessageBuilder;
use eyre::Result;
use rand::{Rng, random, thread_rng};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::guild::Permissions;

use crate::{
    core::commands::{CommandOrigin, prefix::ArgsNum},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

const DEFAULT_LIMIT: u32 = 100;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "roll", desc = "Roll a random number")]
#[flags(SKIP_DEFER)]
pub struct Roll {
    #[command(desc = "Specify an upper limit or `random`, defaults to 100")]
    limit: Option<String>,
}

async fn slash_roll(mut command: InteractionCommand) -> Result<()> {
    let args = Roll::from_interaction(command.input_data())?;

    let limit = match args.limit.as_deref() {
        Some("random" | "?") => None,
        Some(n) => Some(n.parse().unwrap_or(DEFAULT_LIMIT)),
        None => Some(DEFAULT_LIMIT),
    };

    roll((&mut command).into(), limit).await
}

#[command]
#[desc("Get a random number")]
#[help(
    "Get a random number.\n\
    If no upper limit is specified, it defaults to 100."
)]
#[usage("[upper limit]")]
#[flags(SKIP_DEFER)]
#[group(Utility)]
async fn prefix_roll(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let limit = match args.num {
        ArgsNum::Value(n) => Some(n),
        ArgsNum::Random => None,
        ArgsNum::None => match args.next().map(str::parse) {
            Some(Ok(n)) => Some(n),
            None | Some(Err(_)) => Some(DEFAULT_LIMIT),
        },
    };

    roll(CommandOrigin::from_msg(msg, permissions), limit).await
}

async fn roll(orig: CommandOrigin<'_>, limit: Option<u32>) -> Result<()> {
    let author_id = orig.user_id()?;
    let num = limit.map_or_else(random, |limit| thread_rng().gen_range(1..=limit.max(2)));

    let description = format!(
        "<@{author_id}> rolls {num} point{} :game_die:",
        if num == 1 { "" } else { "s" }
    );

    let builder = MessageBuilder::new().embed(description);
    orig.callback(builder).await?;

    Ok(())
}
