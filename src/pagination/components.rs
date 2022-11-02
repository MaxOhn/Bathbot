use std::sync::Arc;

use eyre::{ContextCompat, Report, Result, WrapErr};

use crate::{
    core::Context,
    util::{
        builder::{MessageBuilder, ModalBuilder},
        interaction::{InteractionComponent, InteractionModal},
        Authored, ComponentExt, ModalExt,
    },
};

use super::Pages;

pub(super) async fn remove_components(
    ctx: &Context,
    component: &InteractionComponent,
) -> Result<()> {
    let builder = MessageBuilder::new()
        .components(Vec::new())
        .content(&component.message.content);

    component.callback(ctx, builder).await?;

    Ok(())
}

pub async fn handle_pagination_component<F>(
    ctx: Arc<Context>,
    component: InteractionComponent,
    page_fn: F,
) -> Result<()>
where
    F: FnOnce(&mut Pages),
{
    let (builder, defer_components) = {
        let mut guard = ctx.paginations.lock(&component.message.id).await;

        if let Some(pagination) = guard.get_mut() {
            if !pagination.is_author(component.user_id()?) {
                return Ok(());
            }

            let defer_components = pagination.defer_components;

            if defer_components {
                component.defer(&ctx).await.wrap_err("failed to defer")?;
            }

            pagination.reset_timeout();
            page_fn(&mut pagination.pages);

            (pagination.build(&ctx).await, defer_components)
        } else {
            return remove_components(&ctx, &component).await;
        }
    };

    if defer_components {
        component
            .update(&ctx, &builder?)
            .await
            .wrap_err("failed to update")?;
    } else {
        component
            .callback(&ctx, builder?)
            .await
            .wrap_err("failed to callback")?;
    }

    Ok(())
}

pub async fn handle_pagination_start(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let f = |pages: &mut Pages| pages.index = 0;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_back(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let f = |pages: &mut Pages| pages.index -= pages.per_page;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_step(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let f = |pages: &mut Pages| pages.index += pages.per_page;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_end(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let f = |pages: &mut Pages| pages.index = pages.last_index;

    handle_pagination_component(ctx, component, f).await
}

pub async fn handle_pagination_custom(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let max_page = {
        let guard = ctx.paginations.lock(&component.message.id).await;

        if let Some(pagination) = guard.get() {
            if !pagination.is_author(component.user_id()?) {
                return Ok(());
            }

            pagination.reset_timeout();

            pagination.pages.last_page()
        } else {
            return remove_components(&ctx, &component).await;
        }
    };

    let placeholder = format!("Number between 1 and {max_page}");

    let modal = ModalBuilder::new("page_input", "Page number")
        .modal_id("pagination_page")
        .min_len(1)
        .max_len(5)
        .placeholder(placeholder)
        .title("Jump to a page");

    component
        .modal(&ctx, modal)
        .await
        .wrap_err("failed modal callback")?;

    Ok(())
}

pub async fn handle_pagination_modal(ctx: Arc<Context>, modal: InteractionModal) -> Result<()> {
    let input = modal
        .data
        .components
        .first()
        .and_then(|row| row.components.first())
        .wrap_err("missing page input")?;

    let page: usize = if let Some(Ok(n)) = input.value.as_deref().map(str::parse) {
        n
    } else {
        debug!("failed to parse page input `{:?}` as usize", input.value);

        return Ok(());
    };

    let (builder, defer_components) = if let Some(ref msg) = modal.message {
        let mut guard = ctx.paginations.lock(&msg.id).await;

        if let Some(pagination) = guard.get_mut() {
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
                modal.defer(&ctx).await.wrap_err("failed to defer")?;
            }

            pagination.reset_timeout();
            pagination.pages.index = (page - 1) * pagination.pages.per_page;

            (pagination.build(&ctx).await, defer_components)
        } else {
            return Ok(());
        }
    } else {
        warn!(
            "received unexpected modal (has msg: {})",
            modal.message.is_some()
        );

        return Ok(());
    };

    if defer_components {
        modal
            .update(&ctx, &builder?)
            .await
            .wrap_err("failed to update")?;
    } else {
        modal
            .callback(&ctx, builder?)
            .await
            .wrap_err("failed to callback")?;
    }

    Ok(())
}

pub async fn handle_profile_menu(
    ctx: Arc<Context>,
    mut component: InteractionComponent,
) -> Result<()> {
    let value = component
        .data
        .values
        .pop()
        .wrap_err("missing value for profile menu")?;

    let idx = match value.as_str() {
        "compact" => 0,
        "user_stats" => 1,
        "top100_stats" => 2,
        "top100_mods" => 3,
        "top100_mappers" => 4,
        "mapper_stats" => 5,
        _ => {
            return Err(Report::msg(format!(
                "unknown profile menu option `{value}`"
            )))
        }
    };

    let f = |pages: &mut Pages| pages.index = idx;

    handle_pagination_component(ctx, component, f).await
}
