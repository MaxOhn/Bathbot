use std::{fmt::Write, mem, sync::Arc};

use bathbot_util::{
    constants::{GENERAL_ISSUE, RED},
    EmbedBuilder, FooterBuilder, MessageBuilder,
};
use eyre::{ContextCompat, Result, WrapErr};
use tokio::sync::oneshot;
use twilight_model::channel::message::embed::EmbedField;

use super::{
    retry::{await_retry, RetryState},
    HlComponents, HlGuess,
};
use crate::{
    core::Context,
    games::hl::GameState,
    util::{interaction::InteractionComponent, Authored, ComponentExt},
};

/// Higher Button
pub async fn handle_higher(ctx: Arc<Context>, component: InteractionComponent) -> Result<()> {
    handle_higherlower(ctx, component, HlGuess::Higher).await
}

/// Lower Button
pub async fn handle_lower(ctx: Arc<Context>, component: InteractionComponent) -> Result<()> {
    handle_higherlower(ctx, component, HlGuess::Lower).await
}

async fn handle_higherlower(
    ctx: Arc<Context>,
    component: InteractionComponent,
    guess: HlGuess,
) -> Result<()> {
    let user = component.user_id()?;

    let is_correct = if let Some(game) = ctx.hl_games().lock(&user).await.get() {
        if game.msg != component.message.id {
            return Ok(());
        }

        Some(game.check_guess(guess))
    } else {
        None
    };

    match is_correct {
        Some(true) => correct_guess(ctx, component, guess).await?,
        Some(false) => game_over(ctx, component, guess).await?,
        None => {}
    }

    Ok(())
}

/// Next Button
pub async fn handle_next_higherlower(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let user = component.user_id()?;

    let embed = {
        if let Some(game) = ctx.hl_games().lock(&user).await.get_mut() {
            let components = HlComponents::disabled();
            let builder = MessageBuilder::new().components(components);

            let callback_fut = component.callback(&ctx, builder);
            let embed_fut = game.make_embed();

            let (callback_res, embed) = tokio::join!(callback_fut, embed_fut);
            callback_res.wrap_err("failed to callback")?;

            Some(embed)
        } else {
            None
        }
    };

    if let Some(embed) = embed {
        let components = HlComponents::higherlower();
        let builder = MessageBuilder::new().embed(embed).components(components);

        component
            .update(&ctx, &builder)
            .wrap_err("lacking permission to update message")?
            .await
            .wrap_err("failed to update")?;
    }

    Ok(())
}

/// Try again Button
pub async fn handle_try_again(
    ctx: Arc<Context>,
    mut component: InteractionComponent,
) -> Result<()> {
    let user = component.user_id()?;
    let msg = component.message.id;

    let available_game = {
        let guard = ctx.hl_retries().lock(&msg);

        guard.get().filter(|game| user == game.user).is_some()
    };

    if !available_game {
        return Ok(());
    }

    let mut embeds = mem::take(&mut component.message.embeds);

    let game_fut = if let Some(retry) = ctx.hl_retries().lock(&msg).remove() {
        let _ = retry.tx.send(());

        retry.game.restart(&ctx, &component)
    } else {
        return Ok(());
    };

    let mut embed = embeds.pop().wrap_err("missing embed")?;
    let footer = FooterBuilder::new("Preparing game, give me a moment...");
    embed.footer = Some(footer.build());

    let components = HlComponents::disabled();
    let builder = MessageBuilder::new().embed(embed).components(components);

    component
        .callback(&ctx, builder)
        .await
        .wrap_err("failed to callback")?;

    let mut game = match game_fut.await {
        Ok(game) => game,
        Err(err) => {
            // ? Should ComponentExt provide error method?
            let embed = EmbedBuilder::new().description(GENERAL_ISSUE).color(RED);
            let builder = MessageBuilder::new().embed(embed);

            if let Some(update_fut) = component.update(&ctx, &builder) {
                let _ = update_fut.await;
            }

            return Err(err.wrap_err("failed to restart game"));
        }
    };

    let embed = game.make_embed().await;
    let components = HlComponents::higherlower();
    let builder = MessageBuilder::new().embed(embed).components(components);

    let response = component
        .update(&ctx, &builder)
        .wrap_err("lacking permission to update message")?
        .await
        .wrap_err("failed to update")?
        .model()
        .await
        .wrap_err("failed to deserialize response")?;

    game.msg = response.id;
    ctx.hl_games().own(user).await.insert(game);

    Ok(())
}

async fn correct_guess(
    ctx: Arc<Context>,
    mut component: InteractionComponent,
    guess: HlGuess,
) -> Result<()> {
    let user = component.user_id()?;
    let components = HlComponents::disabled();
    let ctx_clone = Arc::clone(&ctx);

    let embed = if let Some(mut game) = ctx.hl_games().lock(&user).await.get_mut() {
        // Callback with disabled components so nothing happens while the game is
        // updated
        let builder = MessageBuilder::new().components(components);
        component
            .callback(&ctx, builder)
            .await
            .wrap_err("failed to callback")?;

        // Update current score in embed
        let mut embed = game.reveal(&mut component).wrap_err("failed to reveal")?;

        game.current_score += 1;
        let mut footer = game.footer();
        let _ = write!(footer, " • {guess} was correct, press Next to continue");

        if let Some(footer_) = embed.footer.as_mut() {
            footer_.text = footer;
        }

        // Updated the game
        game.next(ctx_clone)
            .await
            .wrap_err("failed to get next game")?;

        Some(embed)
    } else {
        None
    };

    if let Some(embed) = embed {
        // Send updated embed
        let builder = MessageBuilder::new()
            .embed(embed)
            .components(HlComponents::next());

        component
            .update(&ctx, &builder)
            .wrap_err("lacking permission to update message")?
            .await
            .wrap_err("failed to update")?;
    }

    Ok(())
}

async fn game_over(
    ctx: Arc<Context>,
    mut component: InteractionComponent,
    guess: HlGuess,
) -> Result<()> {
    let user = component.user_id()?;

    let game = if let Some(game) = ctx.hl_games().lock(&user).await.remove() {
        game
    } else {
        return Ok(());
    };

    let GameState {
        current_score,
        highscore,
        ..
    } = &game;

    let value = if game.new_highscore(&ctx, user).await? {
        format!("You achieved a total score of {current_score}, your new personal best :tada:")
    } else {
        format!("You achieved a total score of {current_score}, your personal best is {highscore}.")
    };

    let name = format!("Game Over - {guess} was incorrect");
    let mut embed = game.reveal(&mut component).wrap_err("failed to reveal")?;
    embed.footer.take();

    let field = EmbedField {
        inline: false,
        name,
        value,
    };

    embed.fields.push(field);
    let components = HlComponents::restart();
    let builder = MessageBuilder::new().embed(embed).components(components);

    component
        .callback(&ctx, builder)
        .await
        .wrap_err("failed to callback")?;

    let (tx, rx) = oneshot::channel();
    let msg = game.msg;
    let channel = game.channel;
    let retry = RetryState::new(game, user, tx);
    ctx.hl_retries().own(msg).insert(retry);
    tokio::spawn(await_retry(ctx, msg, channel, rx));

    Ok(())
}
