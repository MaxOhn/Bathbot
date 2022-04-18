use std::{fmt::Write, mem, sync::Arc};

use tokio::sync::oneshot;
use twilight_model::{
    application::interaction::MessageComponentInteraction, channel::embed::EmbedField,
};

use crate::{
    core::Context,
    error::InvalidGameState,
    games::hl::GameState,
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        Authored, ComponentExt, MessageExt,
    },
    BotResult,
};

use super::{
    retry::{await_retry, RetryState},
    HlComponents, HlGuess,
};

/// Higher Button
pub async fn handle_higher(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    handle_higherlower(ctx, component, HlGuess::Higher).await
}

/// Lower Button
pub async fn handle_lower(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    handle_higherlower(ctx, component, HlGuess::Lower).await
}

async fn handle_higherlower(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
    guess: HlGuess,
) -> BotResult<()> {
    let user = component.user_id()?;

    let is_correct = if let Some(game) = ctx.hl_games().lock().await.get(&user) {
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

/// Give up Button
pub async fn handle_give_up(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    component.defer(&ctx).await?;
    let user = component.user_id()?;

    let game = if let Some(game) = ctx.hl_games().lock().await.remove(&user) {
        game
    } else {
        return Ok(());
    };

    let mut embed = component
        .message
        .embeds
        .pop()
        .ok_or(InvalidGameState::MissingEmbed)?;

    let footer = FooterBuilder::new("Preparing game, give me a moment...");
    embed.footer = Some(footer.build());

    let components = HlComponents::disabled();
    let update_builder = MessageBuilder::new().embed(embed).components(components);
    let update_fut = component.update(&ctx, &update_builder);

    let components = HlComponents::disabled();
    let disable_builder = MessageBuilder::new().components(components);
    let disable_fut = (game.msg, game.channel).update(&ctx, &disable_builder);

    let (msg_res, _) = tokio::try_join!(update_fut, disable_fut)?;
    let msg = msg_res.model().await?;

    let mut game = match game.restart(&ctx, &msg).await {
        Ok(game) => game,
        Err(err) => {
            let embed = EmbedBuilder::new().description(GENERAL_ISSUE).color(RED);
            let builder = MessageBuilder::new().embed(embed);
            let _ = msg.update(&ctx, &builder).await;

            return Err(err);
        }
    };

    let embed = game.to_embed().await;
    let components = HlComponents::higherlower();
    let builder = MessageBuilder::new().embed(embed).components(components);

    msg.update(&ctx, &builder).await?;
    game.msg = msg.id;
    ctx.hl_games().lock().await.insert(user, game);

    Ok(())
}

/// Next Button
pub async fn handle_next_higherlower(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    let embed = {
        let mut hl_games = ctx.hl_games().lock().await;

        if let Some(game) = hl_games.get_mut(&user) {
            let components = HlComponents::disabled();
            let builder = MessageBuilder::new().components(components);

            let callback_fut = component.callback(&ctx, builder);
            let embed_fut = game.to_embed();

            let (callback_res, embed) = tokio::join!(callback_fut, embed_fut);
            callback_res?;

            Some(embed)
        } else {
            None
        }
    };

    if let Some(embed) = embed {
        let components = HlComponents::higherlower();
        let builder = MessageBuilder::new().embed(embed).components(components);
        component.update(&ctx, &builder).await?;
    }

    Ok(())
}

/// Try again Button
pub async fn handle_try_again(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;
    let msg = component.message.id;

    let available_game = ctx
        .hl_retries()
        .get(&msg)
        .filter(|game| user == game.user)
        .is_some();

    if !available_game {
        return Ok(());
    }

    let mut embeds = mem::take(&mut component.message.embeds);

    let game_fut = if let Some((_, retry)) = ctx.hl_retries().remove(&msg) {
        let _ = retry.tx.send(());

        retry.game.restart(&ctx, &*component)
    } else {
        return Ok(());
    };

    let mut embed = embeds.pop().ok_or(InvalidGameState::MissingEmbed)?;
    let footer = FooterBuilder::new("Preparing game, give me a moment...");
    embed.footer = Some(footer.build());

    let components = HlComponents::disabled();
    let builder = MessageBuilder::new().embed(embed).components(components);

    component.callback(&ctx, builder).await?;

    let mut game = match game_fut.await {
        Ok(game) => game,
        Err(err) => {
            // ? Should ComponentExt provide error method?
            let embed = EmbedBuilder::new().description(GENERAL_ISSUE).color(RED);
            let builder = MessageBuilder::new().embed(embed);
            let _ = component.update(&ctx, &builder).await;

            return Err(err);
        }
    };

    let embed = game.to_embed().await;
    let components = HlComponents::higherlower();
    let builder = MessageBuilder::new().embed(embed).components(components);

    let response = component.update(&ctx, &builder).await?.model().await?;
    game.msg = response.id;
    ctx.hl_games().lock().await.insert(user, game);

    Ok(())
}

async fn correct_guess(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
    guess: HlGuess,
) -> BotResult<()> {
    let user = component.user_id()?;
    let components = HlComponents::next();
    let ctx_clone = Arc::clone(&ctx);
    let mut hl_games = ctx.hl_games().lock().await;

    if let Some(mut game) = hl_games.get_mut(&user) {
        let mut embed = game.reveal(&mut component)?;

        // Update current score
        game.current_score += 1;
        let mut footer = game.footer();
        let _ = write!(footer, " • {guess} was correct, press Next to continue");

        if let Some(footer_) = embed.footer.as_mut() {
            footer_.text = footer;
        }

        let builder = MessageBuilder::new().embed(embed).components(components);

        let callback_fut = component.callback(&ctx, builder);
        let next_fut = game.next(ctx_clone);

        let (callback_res, next_res) = tokio::join!(callback_fut, next_fut);

        callback_res?;
        next_res?;
    }

    Ok(())
}

async fn game_over(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
    guess: HlGuess,
) -> BotResult<()> {
    let user = component.user_id()?;

    let game = if let Some(game) = ctx.hl_games().lock().await.remove(&user) {
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
    let mut embed = game.reveal(&mut component)?;
    embed.footer.take();

    let field = EmbedField {
        inline: false,
        name,
        value,
    };

    embed.fields.push(field);
    let components = HlComponents::restart();
    let builder = MessageBuilder::new().embed(embed).components(components);

    component.callback(&ctx, builder).await?;

    let (tx, rx) = oneshot::channel();
    let msg = game.msg;
    let channel = game.channel;
    let retry = RetryState::new(game, user, tx);
    ctx.hl_retries().insert(msg, retry);
    tokio::spawn(await_retry(ctx, msg, channel, rx));

    Ok(())
}
