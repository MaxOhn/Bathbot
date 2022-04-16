use command_macros::SlashCommand;
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{
        component::{button::ButtonStyle, ActionRow, Button, Component},
        interaction::ApplicationCommand,
    },
    id::Id,
};

use crate::{
    games::hl::{hl_components, random_play, GameState},
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        ApplicationCommandExt, Authored,
    },
    BotResult, Context,
};

use std::sync::Arc;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "higherlower",
    help = "Play a game of osu! themed higher lower.\n\
    The available modes are:\n \
    - `PP`: Guess whether the next play is worth higher or lower PP!"
)]
/// Play a game of osu! themed higher lower
pub struct HigherLower;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "hl",
    help = "Play a game of osu! themed higher lower.\n\
    The available modes are:\n \
    - `PP`: Guess whether the next play is worth higher or lower PP!"
)]
/// Play a game of osu! themed higher lower
pub struct Hl;

async fn slash_higherlower(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    // TODO: handle modes, add different modes, add difficulties and difficulty increase
    let user = command.user_id()?;
    let content = ctx.hl_games().get(&user).map(|v| {
        let GameState { guild, channel, id, .. } = v.value();

        format!(
            "You can't play two higher lower games at once! \n\
            Finish your [other game](https://discord.com/channels/{}/{channel}/{id}) first or give up.",
            match guild {
                Some(id) => id.to_string(),
                None => "@me".to_string(),
            },
        )
    });

    if let Some(content) = content {
        let components = give_up_components();
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        let builder = MessageBuilder::new().embed(embed).components(components);
        command.update(&ctx, &builder).await?;
    } else {
        let (play1, mut play2) =
            match tokio::try_join!(random_play(&ctx, 0.0, 0), random_play(&ctx, 0.0, 0)) {
                Ok(tuple) => tuple,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;
                    return Err(err);
                }
            };

        while play2 == play1 {
            play2 = random_play(&ctx, 0.0, 0).await?;
        }

        //TODO: handle mode
        let mut game = GameState {
            previous: play1,
            next: play2,
            player: user,
            id: Id::new(1),
            channel: command.channel_id(),
            guild: command.guild_id(),
            mode: 1,
            current_score: 0,
            highscore: ctx.psql().get_higherlower_highscore(user.get(), 1).await?,
        };

        let image = game.create_image(&ctx).await?;
        let components = hl_components();
        let embed = game.to_embed(image);

        let builder = MessageBuilder::new().embed(embed).components(components);
        let response = command.update(&ctx, &builder).await?.model().await?;

        game.id = response.id;
        ctx.hl_games().insert(user, game);
    }

    Ok(())
}

async fn slash_hl(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    slash_higherlower(ctx, command).await
}

fn give_up_components() -> Vec<Component> {
    let give_up_button = Button {
        custom_id: Some("give_up_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Give Up".to_owned()),
        style: ButtonStyle::Danger,
        url: None,
    };

    let button_row = ActionRow {
        components: vec![Component::Button(give_up_button)],
    };

    vec![Component::ActionRow(button_row)]
}
