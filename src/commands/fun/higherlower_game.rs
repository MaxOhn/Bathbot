use command_macros::SlashCommand;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    commands::GameModeOption,
    games::hl::{GameState, HlComponents, HlVersion},
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        ApplicationCommandExt, Authored,
    },
    BotResult, Context,
};

use std::{fmt::Display, sync::Arc};

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

    let content = ctx.hl_games().get(&user).map(|v| {
        let GameState { guild, channel, id, .. } = v.value();

        format!(
            "You can't play two higherlower games at once! \n\
            Finish your [other game](https://discord.com/channels/{}/{channel}/{id}) first or give up.",
            match guild {
                Some(ref id) => id as &dyn Display,
                None => &"@me" as &dyn Display,
            },
        )
    });

    if let Some(content) = content {
        let components = HlComponents::give_up();
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        let builder = MessageBuilder::new().embed(embed).components(components);
        command.update(&ctx, &builder).await?;
    } else {
        let args = HigherLower::from_interaction(command.input_data())?;
        let version = HlVersion::ScorePp;

        let mode = match args.mode.map(GameMode::from) {
            Some(mode) => mode,
            None => ctx.user_config(user).await?.mode.unwrap_or(GameMode::STD),
        };

        let mut game = match GameState::new(&ctx, &*command, mode, version).await {
            Ok(game) => game,
            Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        let image = game.image().await;
        let embed = game.to_embed(image);

        let components = HlComponents::higherlower();
        let builder = MessageBuilder::new().embed(embed).components(components);

        let response = command.update(&ctx, &builder).await?.model().await?;

        game.id = response.id;
        ctx.hl_games().insert(user, game);
    }

    Ok(())
}
