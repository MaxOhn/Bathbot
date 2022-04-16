use std::{fmt::Write, mem, sync::Arc, time::Duration};

use dashmap::mapref::entry::Entry;
use eyre::Report;
use tokio::time::sleep;
use twilight_model::{
    application::{
        component::{button::ButtonStyle, ActionRow, Button, Component},
        interaction::MessageComponentInteraction,
    },
    id::Id,
};

use crate::{
    core::Context,
    games::hl::{hl_components, random_play, GameState},
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        numbers::round,
        Authored, ChannelExt, ComponentExt,
    },
    BotResult,
};

use super::HlGuess;

pub async fn handle_higher(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        let game = entry.get_mut();

        if game.id != component.message.id {
            return Ok(());
        }

        defer_update(&ctx, &mut component, Some(game)).await?;

        if !game.check_guess(HlGuess::Higher) {
            game_over(&ctx, &component, game).await?;
            entry.remove();
        } else {
            correct_guess(&ctx, &component, game).await?;
        }
    }

    Ok(())
}

pub async fn handle_lower(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        let game = entry.get_mut();

        if game.id != component.message.id {
            return Ok(());
        }

        defer_update(&ctx, &mut component, Some(game)).await?;

        if !game.check_guess(HlGuess::Lower) {
            game_over(&ctx, &component, game).await?;
            entry.remove();
        } else {
            correct_guess(&ctx, &component, game).await?;
        }
    }

    Ok(())
}

#[allow(unused)]
pub async fn handle_give_up(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;
    if let Some((_, game)) = ctx.hl_games().remove(&user) {
        defer_update(&ctx, &mut component, Some(&game)).await?;

        let content = "Successfully ended the previous game.\n\
                            Start a new game by using `/higherlower`";
        let embed = EmbedBuilder::new().description(content).build();
        let builder = MessageBuilder::new().embed(embed).components(Vec::new());
        component.update(&ctx, &builder).await?;
    }

    Ok(())
}

// TODO
#[allow(unused)]
// TODO: people who didn't run the command can press try again on another game to take over leading to undesirable behaviour
pub async fn handle_try_again(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    // TODO: handle modes, add different modes, add difficulties and difficulty increase
    defer_update(&ctx, &mut component, None).await?;
    let user = component.user_id()?;
    info!("{}, {}", user, component.message.author.id);

    let (play1, mut play2) =
        match tokio::try_join!(random_play(&ctx, 0.0, 0), random_play(&ctx, 0.0, 0)) {
            Ok(tuple) => tuple,
            Err(err) => {
                let _ = component.message.error(&ctx, GENERAL_ISSUE).await;
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
        channel: component.channel_id(),
        guild: component.guild_id(),
        mode: 1,
        current_score: 0,
        highscore: ctx.psql().get_higherlower_highscore(user.get(), 1).await?,
    };

    let image = game.create_image(&ctx).await?;
    let components = hl_components();
    let embed = game.to_embed(image);

    let builder = MessageBuilder::new().embed(embed).components(components);
    let response = component.update(&ctx, &builder).await?.model().await?;
    game.id = response.id;
    ctx.hl_games().insert(user, game);

    Ok(())
}

async fn correct_guess(
    ctx: &Context,
    component: &MessageComponentInteraction,
    game: &mut GameState,
) -> BotResult<()> {
    mem::swap(&mut game.previous, &mut game.next);
    game.next = random_play(ctx, game.previous.pp, game.current_score).await?;

    while game.next == game.previous {
        game.next = random_play(ctx, game.previous.pp, game.current_score).await?;
    }

    game.current_score += 1;

    let image = match game.create_image(ctx).await {
        Ok(url) => url,
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to create hl image");
            warn!("{report:?}");

            String::new()
        }
    };

    let embed = game.to_embed(image);
    let builder = MessageBuilder::new().embed(embed);
    component.update(ctx, &builder).await?;

    Ok(())
}

async fn game_over(
    ctx: &Context,
    component: &MessageComponentInteraction,
    game: &GameState,
) -> BotResult<()> {
    let score_fut = ctx.psql().upsert_higherlower_highscore(
        game.player.get(),
        game.mode,
        game.current_score,
        game.highscore,
    );

    let better_score = score_fut.await?;
    let title = "Game over!";

    let content = match better_score {
        true => {
            format!(
                "You achieved a total score of {}! \nThis is your new personal best!",
                game.current_score
            )
        }
        false => {
            format!(
                "You achieved a total score of {}! \n\
                This unfortunately did not beat your personal best score of {}!",
                game.current_score, game.highscore
            )
        }
    };

    let embed = EmbedBuilder::new()
        .title(title)
        .description(content)
        .color(RED)
        .build();

    //TODO: length might change based on release speed
    sleep(Duration::from_secs(2)).await;
    let components = try_again_components();
    let builder = MessageBuilder::new().embed(embed).components(components);
    component.update(ctx, &builder).await?;

    Ok(())
}

//TODO: show red bar if they get it wrong to easily see if you got it wrong
async fn defer_update(
    ctx: &Context,
    component: &mut MessageComponentInteraction,
    game: Option<&GameState>,
) -> BotResult<()> {
    let mut embeds = mem::take(&mut component.message.embeds);
    if let Some(embed) = embeds.first_mut() {
        if let Some(game) = game {
            if let Some(field) = embed.fields.get_mut(1) {
                field.value.truncate(field.value.len() - 7);
                let _ = write!(field.value, "{}pp**", round(game.next.pp));
            }
            if let Some(footer) = embed.footer.as_mut() {
                let _ = write!(
                    footer.text,
                    " • {}pp {} • Retrieving next play...",
                    round((game.previous.pp - game.next.pp).abs()),
                    if game.previous.pp < game.next.pp {
                        "higher"
                    } else {
                        "lower"
                    }
                );
            }
        }
    }

    let builder = MessageBuilder::new().embed(embeds.pop().unwrap()); // TODO
    component.callback(&ctx, builder).await?;

    Ok(())
}

fn try_again_components() -> Vec<Component> {
    let try_again_button = Button {
        custom_id: Some("try_again_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Try Again".to_owned()),
        style: ButtonStyle::Success,
        url: None,
    };

    let button_row = ActionRow {
        components: vec![Component::Button(try_again_button)],
    };

    vec![Component::ActionRow(button_row)]
}
