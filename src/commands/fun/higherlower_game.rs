use command_macros::SlashCommand;
use twilight_interactions::command::CreateCommand;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    games::hl::{GameState, HigherLowerComponents, HlVersion},
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        ApplicationCommandExt, Authored,
    },
    BotResult, Context,
};

use std::{fmt::Display, sync::Arc};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "higherlower",
    help = "Play a game of osu! themed higher lower.\n\
    The available versions are:\n \
    - `Score PP`: Guess whether the next play is worth higher or lower PP"
)]
/// Play a game of osu! themed higher lower
pub struct HigherLower;

async fn slash_higherlower(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let user = command.user_id()?;

    let content = ctx.hl_games().get(&user).map(|v| {
        let GameState { guild, channel, id, .. } = v.value();

        format!(
            "You can't play two higher lower games at once! \n\
            Finish your [other game](https://discord.com/channels/{}/{channel}/{id}) first or give up.",
            match guild {
                Some(ref id) => id as &dyn Display,
                None => &"@me" as &dyn Display,
            },
        )
    });

    if let Some(content) = content {
        let components = HigherLowerComponents::give_up();
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        let builder = MessageBuilder::new().embed(embed).components(components);
        command.update(&ctx, &builder).await?;
    } else {
        let highscore = ctx
            .psql()
            .get_higherlower_highscore(user.get(), HlVersion::ScorePp)
            .await?;

        let mut game = match GameState::new(&ctx, &*command, highscore).await {
            Ok(game) => game,
            Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        let image = game.image().await;
        let embed = game.to_embed(image);

        let components = HigherLowerComponents::new()
            .disable_next()
            .disable_restart();

        let builder = MessageBuilder::new()
            .embed(embed)
            .components(components.into());

        let response = command.update(&ctx, &builder).await?.model().await?;

        game.id = response.id;
        ctx.hl_games().insert(user, game);
    }

    Ok(())
}
