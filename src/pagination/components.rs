use std::sync::Arc;

use twilight_model::application::interaction::{
    modal::ModalSubmitInteraction, MessageComponentInteraction,
};

use crate::{
    core::Context,
    error::InvalidModal,
    util::{
        builder::{MessageBuilder, ModalBuilder},
        Authored, ComponentExt, ModalExt,
    },
    BotResult,
};

use super::Pages;

pub(super) async fn remove_components(
    ctx: &Context,
    component: &MessageComponentInteraction,
) -> BotResult<()> {
    let builder = MessageBuilder::new()
        .components(Vec::new())
        .content(&component.message.content);

    component.callback(ctx, builder).await?;

    Ok(())
}

pub async fn handle_pagination_component(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
    page_fn: fn(&mut Pages),
) -> BotResult<()> {
    let (builder, defer_components) =
        if let Some(mut pagination) = ctx.paginations.get_mut(&component.message.id) {
            if !pagination.is_author(component.user_id()?) {
                return Ok(());
            }

            let defer_components = pagination.defer_components;

            if defer_components {
                component.defer(&ctx).await?;
            }

            pagination.reset_timeout();
            page_fn(&mut pagination.pages);

            (pagination.build(&ctx).await, defer_components)
        } else {
            return remove_components(&ctx, &component).await;
        };

    if defer_components {
        component.update(&ctx, &builder?).await?;
    } else {
        component.callback(&ctx, builder?).await?;
    }

    Ok(())
}

pub async fn handle_pagination_start(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index = 0;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_back(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index -= pages.per_page;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_step(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index += pages.per_page;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_end(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index = pages.last_index;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_custom(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let max_page = if let Some(pagination) = ctx.paginations.get(&component.message.id) {
        if !pagination.is_author(component.user_id()?) {
            return Ok(());
        }

        pagination.reset_timeout();

        pagination.pages.last_page()
    } else {
        return remove_components(&ctx, &component).await;
    };

    let placeholder = format!("Number between 1 and {max_page}");

    let modal = ModalBuilder::new("page_input", "Page number")
        .modal_id("pagination_page")
        .min_len(1)
        .max_len(5)
        .placeholder(placeholder)
        .title("Jump to a page");

    component.modal(&ctx, modal).await?;

    Ok(())
}

pub async fn handle_pagination_modal(
    ctx: Arc<Context>,
    modal: Box<ModalSubmitInteraction>,
) -> BotResult<()> {
    let input = modal
        .data
        .components
        .first()
        .ok_or(InvalidModal::MissingPageInput)?
        .components
        .first()
        .ok_or(InvalidModal::MissingPageInput)?;

    let page: usize = if let Ok(n) = input.value.parse() {
        n
    } else {
        debug!("failed to parse page input `{}` as usize", input.value);

        return Ok(());
    };

    let (builder, defer_components) = if let Some(mut pagination) = modal
        .message
        .as_ref()
        .and_then(|msg| ctx.paginations.get_mut(&msg.id))
    {
        if !pagination.is_author(modal.user_id()?) {
            return Ok(());
        }

        let max_page = pagination.pages.last_page();

        if !(1..=max_page).contains(&page) {
            debug!("page {page} is not between 1 and {max_page}");

            return Ok(());
        }

        let defer_components = pagination.defer_components;

        if defer_components {
            modal.defer(&ctx).await?;
        }

        pagination.reset_timeout();
        pagination.pages.index = (page - 1) * pagination.pages.per_page;

        (pagination.build(&ctx).await, defer_components)
    } else {
        warn!(
            "received unexpected modal (has msg: {})",
            modal.message.is_some()
        );

        return Ok(());
    };

    if defer_components {
        modal.update(&ctx, &builder?).await?;
    } else {
        modal.callback(&ctx, builder?).await?;
    }

    Ok(())
}

pub async fn handle_profile_compact(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index = 0;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_profile_medium(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index = 1;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_profile_full(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let f = |pages: &mut Pages| pages.index = 2;

    handle_pagination_component(ctx, component, f).await
}
