use command_macros::SlashCommand;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    commands::GameModeOption,
    games::hl::{GameState, HlComponents},
    util::{
        builder::MessageBuilder, constants::GENERAL_ISSUE, ApplicationCommandExt, Authored,
        MessageExt,
    },
    BotResult, Context,
};

use std::sync::Arc;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "higherlower",
    help = "Play a game of osu! themed higher lower.\n\
    The available versions are:\n \
    - `Score PP`: Guess whether the next play is worth higher or lower PP"
)]
/// Play a game of osu! themed higher lower
pub struct HigherLower {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
}

async fn slash_higherlower(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let user = command.user_id()?;

    if let Some(game) = ctx.hl_games().lock().await.remove(&user) {
        let components = HlComponents::disabled();
        let builder = MessageBuilder::new().components(components);
        (game.msg, game.channel).update(&ctx, &builder).await?;
    }

    let args = HigherLower::from_interaction(command.input_data())?;

    let mode = match args.mode.map(GameMode::from) {
        Some(mode) => mode,
        None => ctx.user_config(user).await?.mode.unwrap_or(GameMode::STD),
    };

    let mut game = match GameState::score_pp(&ctx, &*command, mode).await {
        Ok(game) => game,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let embed = game.to_embed().await;
    let components = HlComponents::higherlower();
    let builder = MessageBuilder::new().embed(embed).components(components);

    let response = command.update(&ctx, &builder).await?.model().await?;

    game.msg = response.id;
    ctx.hl_games().lock().await.insert(user, game);

    Ok(())
}
