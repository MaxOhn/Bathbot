use std::sync::Arc;

use bathbot_macros::{command, SlashCommand};
use bathbot_util::MessageBuilder;
use eyre::Result;
use rand::Rng;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::guild::Permissions;

use crate::{
    core::{commands::CommandOrigin, Context},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

const DEFAULT_LIMIT: u64 = 100;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "roll")]
#[flags(SKIP_DEFER)]
/// Roll a random number
pub struct Roll {
    #[command(min_value = 1)]
    /// Specify an upper limit, defaults to 100
    limit: Option<i64>,
}

async fn slash_roll(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Roll::from_interaction(command.input_data())?;
    let limit = args.limit.map_or(DEFAULT_LIMIT, |l| l as u64);

    roll(ctx, (&mut command).into(), limit).await
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
    ctx: Arc<Context>,
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let limit = match args.num {
        Some(n) => n,
        None => match args.next().map(|arg| arg.parse()) {
            Some(Ok(n)) => n,
            None | Some(Err(_)) => DEFAULT_LIMIT,
        },
    };

    roll(ctx, CommandOrigin::from_msg(msg, permissions), limit).await
}

async fn roll(ctx: Arc<Context>, orig: CommandOrigin<'_>, limit: u64) -> Result<()> {
    let num = rand::thread_rng().gen_range(1..(limit + 1).max(2));
    let author_id = orig.user_id()?;

    let description = format!(
        "<@{author_id}> rolls {num} point{} :game_die:",
        if num == 1 { "" } else { "s" }
    );

    let builder = MessageBuilder::new().embed(description);
    orig.callback(&ctx, builder).await?;

    Ok(())
}
