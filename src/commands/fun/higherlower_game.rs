use std::sync::Arc;

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

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "higherlower")]
/// Play a game of osu! themed higher lower
pub enum HigherLower {
    #[command(name = "pp")]
    ScorePp(HigherLowerScorePp),
    #[command(name = "farm")]
    FarmMaps(HigherLowerFarmMaps),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "pp",
    help = "Is the score's pp value higher or lower?\n\
    The players are chosen randomly from the top 5,000 and the top score \
    is chosen randomly as well but the higher the current score is, the more \
    likely it is that the next pp value is close to the previous pp."
)]
/// Is the score's pp value higher or lower?
pub struct HigherLowerScorePp {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "farm",
    help = "Is the amount of times the map appears in top scores higher or lower?\n\
    All counts are provided by [osutracker](https://osutracker.com) which only includes a portion \
    of the actual data but it should be representative, at least for >300pp scores.\n\
    The maps are chosen randomly based on [this weight function](https://www.desmos.com/calculator/u4jt9t4jnj)."
)]
/// Is the amount of times the map appears in top scores higher or lower?
pub struct HigherLowerFarmMaps;

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

    let game_res = match args {
        HigherLower::ScorePp(args) => {
            let mode = match args.mode.map(GameMode::from) {
                Some(mode) => mode,
                None => ctx.user_config(user).await?.mode.unwrap_or(GameMode::STD),
            };

            GameState::score_pp(&ctx, &*command, mode).await
        }
        HigherLower::FarmMaps(_) => GameState::farm_maps(&ctx, &*command).await,
    };

    let mut game = match game_res {
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
