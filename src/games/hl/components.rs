use std::{fmt::Write, mem, sync::Arc};

use dashmap::mapref::entry::Entry;
use tokio::sync::oneshot;
use twilight_model::{
    application::interaction::MessageComponentInteraction,
    channel::embed::{Embed, EmbedField},
};

use crate::{
    core::Context,
    error::InvalidGameState,
    games::hl::GameState,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        numbers::round,
        Authored, ComponentExt,
    },
    BotResult,
};

use super::{
    retry::{await_retry, RetryState},
    HigherLowerComponents, HlGuess,
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

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        let game = entry.get_mut();

        if game.id != component.message.id {
            return Ok(());
        }

        if !game.check_guess(guess) {
            let game = entry.remove();
            game_over(Arc::clone(&ctx), component, game, guess).await?;
        } else {
            correct_guess(Arc::clone(&ctx), component, game, guess).await?;
        }
    }

    Ok(())
}

/// Give up Button
pub async fn handle_give_up(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    if ctx.hl_games().remove(&user).is_some() {
        let content = "Successfully ended the previous game.\n\
            Start a new game by using `/higherlower`";

        let builder = MessageBuilder::new().embed(content).components(Vec::new());
        component.callback(&ctx, builder).await?;
    }

    Ok(())
}

/// Next Button
pub async fn handle_next_higherlower(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        component.defer(&ctx).await?;
        let game = entry.get_mut();

        let image = game.image().await;
        let embed = game.to_embed(image);

        let components = HigherLowerComponents::new()
            .disable_next()
            .disable_restart();

        let builder = MessageBuilder::new()
            .embed(embed)
            .components(components.into());

        component.update(&ctx, &builder).await?;
    }

    Ok(())
}

/// Try again Button
pub async fn handle_try_again(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let user = component.user_id()?;

    let version = match ctx.hl_retries().entry(component.message.id) {
        Entry::Occupied(entry) => {
            if user != entry.get().user {
                return Ok(());
            }

            let RetryState { game, tx, .. } = entry.remove();
            let _ = tx.send(());

            game.version
        }
        Entry::Vacant(_) => return Ok(()),
    };

    component.defer(&ctx).await?;

    let mut game = match GameState::new(&ctx, &*component, version).await {
        Ok(game) => game,
        Err(err) => {
            // ? Should ComponentExt provide error method?
            let embed = EmbedBuilder::new().description(GENERAL_ISSUE).color(RED);
            let builder = MessageBuilder::new().embed(embed);
            let _ = component.update(&ctx, &builder).await;

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

    let response = component.update(&ctx, &builder).await?.model().await?;
    game.id = response.id;
    ctx.hl_games().insert(user, game);

    Ok(())
}

async fn correct_guess(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
    game: &mut GameState,
    guess: HlGuess,
) -> BotResult<()> {
    let mut embed = extract_revealed_pp(&mut component, game.next.pp)?;

    // Update current score
    game.current_score += 1;
    let mut footer = game.footer();

    let _ = write!(footer, " • {guess} was correct, press Next to continue");

    if let Some(footer_) = embed.footer.as_mut() {
        footer_.text = footer;
    }

    let components = HigherLowerComponents::new()
        .disable_higherlower()
        .disable_restart();

    let builder = MessageBuilder::new()
        .embed(embed)
        .components(components.into());

    let ctx_clone = Arc::clone(&ctx);

    let callback_fut = component.callback(&ctx, builder);
    let next_fut = game.next(ctx_clone);

    let (callback_result, next_result) = tokio::join!(callback_fut, next_fut);

    callback_result?;
    next_result?;

    Ok(())
}

async fn game_over(
    ctx: Arc<Context>,
    mut component: Box<MessageComponentInteraction>,
    game: GameState,
    guess: HlGuess,
) -> BotResult<()> {
    let user = component.user_id()?;

    let GameState {
        version,
        current_score,
        highscore,
        next,
        ..
    } = &game;

    let better_score_fut =
        ctx.psql()
            .upsert_higherlower_highscore(user.get(), *version, *current_score, *highscore);

    let name = format!("Game Over - {guess} was incorrect");

    let value = if better_score_fut.await? {
        format!("You achieved a total score of {current_score}, your new personal best :tada:")
    } else {
        format!("You achieved a total score of {current_score}, your personal best is {highscore}.")
    };

    let mut embed = extract_revealed_pp(&mut component, next.pp)?;

    embed.footer.take();

    let field = EmbedField {
        inline: false,
        name,
        value,
    };

    embed.fields.push(field);

    let components = HigherLowerComponents::new()
        .disable_higherlower()
        .disable_next();

    let builder = MessageBuilder::new()
        .embed(embed)
        .components(components.into());

    component.callback(&ctx, builder).await?;

    let (tx, rx) = oneshot::channel();
    let msg = game.id;
    let channel = component.channel_id;
    let retry = RetryState::new(game, user, tx);
    ctx.hl_retries().insert(msg, retry);
    tokio::spawn(await_retry(ctx, msg, channel, rx));

    Ok(())
}

fn extract_revealed_pp(component: &mut MessageComponentInteraction, pp: f32) -> BotResult<Embed> {
    let mut embeds = mem::take(&mut component.message.embeds);
    let mut embed = embeds.pop().ok_or(InvalidGameState::MissingEmbed)?;

    if let Some(field) = embed.fields.get_mut(1) {
        field.value.truncate(field.value.len() - 7);
        let _ = write!(field.value, "__{}pp__**", round(pp));
    }

    Ok(embed)
}
