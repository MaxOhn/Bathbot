use std::sync::Arc;

use bathbot_util::{
    modal::{ModalBuilder, TextInputBuilder},
    MessageBuilder,
};
use eyre::{ContextCompat, Report, Result, WrapErr};
use twilight_model::application::interaction::modal::ModalInteractionDataComponent;

use crate::{
    core::Context,
    embeds::TopOldVersion,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Authored, ComponentExt, ModalExt,
    },
};

use super::{ComponentKind, Pages, PaginationKind};

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

        let Some(pagination) = guard.get_mut() else {
            return remove_components(&ctx, &component).await;
        };

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

macro_rules! handle_sim_buttons {
    ( $( $fn:ident, $placeholder:literal, $id:literal, $label:literal, $title:literal ;)* ) => {
        $(
            pub async fn $fn(ctx: Arc<Context>, component: InteractionComponent) -> Result<()> {
                let input = TextInputBuilder::new($id, $label).placeholder($placeholder);
                let modal = ModalBuilder::new($id, $title).input(input);

                component
                    .modal(&ctx, modal)
                    .await
                    .wrap_err("failed modal callback")?;

                Ok(())
            }
        )*
    };
}

handle_sim_buttons! {
    handle_sim_mods_button, "E.g. hd or HdHRdteZ", "sim_mods", "Mods", "Specify mods";
    handle_sim_combo_button, "Integer", "sim_combo", "Combo", "Specify combo";
    handle_sim_acc_button, "Number", "sim_acc", "Accuracy", "Specify an accuracy";
    handle_sim_clock_rate_button, "Number", "sim_clock_rate", "Clock rate", "Specify a clock rate";
    handle_sim_geki_button, "Integer", "sim_geki", "Amount of gekis", "Specify the amount of gekis";
    handle_sim_katu_button, "Integer", "sim_katu", "Amount of katus", "Specify the amount of katus";
    handle_sim_n300_button, "Integer", "sim_n300", "Amount of 300s", "Specify the amount of 300s";
    handle_sim_n100_button, "Integer", "sim_n100", "Amount of 100s", "Specify the amount of 100s";
    handle_sim_n50_button, "Integer", "sim_n50", "Amount of 50s", "Specify the amount of 50s";
    handle_sim_miss_button, "Integer", "sim_miss", "Amount of misses", "Specify the amount of misses";
    handle_sim_score_button, "Integer", "sim_score", "Score", "Specify the score";
}

pub async fn handle_sim_attrs_button(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let ar = TextInputBuilder::new("sim_ar", "AR")
        .placeholder("Specify an approach rate")
        .required(false);

    let cs = TextInputBuilder::new("sim_cs", "CS")
        .placeholder("Specify a circle size")
        .required(false);

    let hp = TextInputBuilder::new("sim_hp", "HP")
        .placeholder("Specify a drain rate")
        .required(false);

    let od = TextInputBuilder::new("sim_od", "OD")
        .placeholder("Specify an overall difficulty")
        .required(false);

    let modal = ModalBuilder::new("sim_attrs", "Attributes")
        .input(ar)
        .input(cs)
        .input(hp)
        .input(od);

    component
        .modal(&ctx, modal)
        .await
        .wrap_err("failed modal callback")?;

    Ok(())
}

pub async fn handle_sim_version(ctx: Arc<Context>, component: InteractionComponent) -> Result<()> {
    let version = component
        .data
        .values
        .first()
        .wrap_err("missing sim version")?;

    let version = TopOldVersion::from_menu_str(version)
        .wrap_err_with(|| format!("unknown TopOldVersion `{version}`"))?;

    let (builder, defer_components) = {
        let mut guard = ctx.paginations.lock(&component.message.id).await;

        let Some(pagination) = guard.get_mut() else {
            return Ok(());
        };

        if !pagination.is_author(component.user_id()?) {
            return Ok(());
        }

        let defer_components = pagination.defer_components;

        if defer_components {
            component.defer(&ctx).await.wrap_err("failed to defer")?;
        }

        pagination.reset_timeout();

        let PaginationKind::Simulate(sim_pagination) = &mut pagination.kind else {
            return Ok(());
        };

        sim_pagination.simulate_data.version = version;
        pagination.component_kind = ComponentKind::Simulate(version);

        (pagination.build(&ctx).await, defer_components)
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

    let input = TextInputBuilder::new("page_input", "Page number")
        .min_len(1)
        .max_len(5)
        .placeholder(placeholder);

    let modal = ModalBuilder::new("pagination_page", "Jump to a page").input(input);

    component
        .modal(&ctx, modal)
        .await
        .wrap_err("failed modal callback")?;

    Ok(())
}

fn parse_modal_input(modal: &InteractionModal) -> Result<&ModalInteractionDataComponent> {
    modal
        .data
        .components
        .first()
        .and_then(|row| row.components.first())
        .wrap_err("missing sim input")
}

async fn respond_modal(
    defer: bool,
    modal: &InteractionModal,
    ctx: &Context,
    builder: Result<MessageBuilder<'_>>,
) -> Result<()> {
    if defer {
        modal
            .update(ctx, &builder?)
            .await
            .wrap_err("failed to update")?;
    } else {
        modal
            .callback(ctx, builder?)
            .await
            .wrap_err("failed to callback")?;
    }

    Ok(())
}

macro_rules! handle_sim_modals {
    ( $( $fn:ident, $setter:ident ;)* ) => {
        $(
            pub async fn $fn(ctx: Arc<Context>, modal: InteractionModal) -> Result<()> {
                let input = parse_modal_input(&modal)?;

                let Some(Ok(value)) = input.value.as_deref().map(str::parse) else {
                    debug!("failed to parse sim input `{:?}`", input.value);

                    return Ok(());
                };

                let Some(ref msg) = modal.message else {
                    warn!("received modal without message");

                    return Ok(());
                };

                let (builder, defer_components) = {
                    let mut guard = ctx.paginations.lock(&msg.id).await;

                    let Some(pagination) = guard.get_mut() else {
                        return Ok(());
                    };

                    if !pagination.is_author(modal.user_id()?) {
                        return Ok(());
                    }

                    let defer_components = pagination.defer_components;

                    if defer_components {
                        modal.defer(&ctx).await.wrap_err("failed to defer")?;
                    }

                    pagination.reset_timeout();

                    let PaginationKind::Simulate(sim_pagination) = &mut pagination.kind else {
                        return Ok(());
                    };

                    sim_pagination.simulate_data.$setter(value);

                    (pagination.build(&ctx).await, defer_components)
                };

                respond_modal(defer_components, &modal, &ctx, builder).await
            }
        )*
    }
}

handle_sim_modals! {
    handle_sim_mods_modal, set_mods;
    handle_sim_combo_modal, set_combo;
    handle_sim_acc_modal, set_acc;
    handle_sim_clock_rate_modal, set_clock_rate;
    handle_sim_geki_modal, set_geki;
    handle_sim_katu_modal, set_katu;
    handle_sim_n300_modal, set_n300;
    handle_sim_n100_modal, set_n100;
    handle_sim_n50_modal, set_n50;
    handle_sim_miss_modal, set_miss;
    handle_sim_score_modal, set_score;
}

pub async fn handle_sim_attrs_modal(ctx: Arc<Context>, modal: InteractionModal) -> Result<()> {
    fn parse_attr(modal: &InteractionModal, component_id: &str) -> Option<f32> {
        modal
            .data
            .components
            .iter()
            .find_map(|row| {
                row.components.first().and_then(|component| {
                    (component.custom_id == component_id).then(|| {
                        component
                            .value
                            .as_deref()
                            .filter(|value| !value.is_empty())
                            .map(str::parse)
                            .and_then(Result::ok)
                    })
                })
            })
            .flatten()
    }

    let ar = parse_attr(&modal, "sim_ar");
    let cs = parse_attr(&modal, "sim_cs");
    let hp = parse_attr(&modal, "sim_hp");
    let od = parse_attr(&modal, "sim_od");

    let Some(ref msg) = modal.message else {
        warn!("received modal without msg");

        return Ok(());
    };

    let (builder, defer_components) = {
        let mut guard = ctx.paginations.lock(&msg.id).await;

        let Some(pagination) = guard.get_mut() else {
            return Ok(());
        };

        if !pagination.is_author(modal.user_id()?) {
            return Ok(());
        }

        let defer_components = pagination.defer_components;

        if defer_components {
            modal.defer(&ctx).await.wrap_err("failed to defer")?;
        }

        pagination.reset_timeout();

        let PaginationKind::Simulate(sim_pagination) = &mut pagination.kind else {
            return Ok(());
        };

        if let Some(ar) = ar {
            sim_pagination.simulate_data.ar = Some(ar);
        }

        if let Some(cs) = cs {
            sim_pagination.simulate_data.cs = Some(cs);
        }

        if let Some(hp) = hp {
            sim_pagination.simulate_data.hp = Some(hp);
        }

        if let Some(od) = od {
            sim_pagination.simulate_data.od = Some(od);
        }

        (pagination.build(&ctx).await, defer_components)
    };

    respond_modal(defer_components, &modal, &ctx, builder).await
}

pub async fn handle_pagination_modal(ctx: Arc<Context>, modal: InteractionModal) -> Result<()> {
    let input = parse_modal_input(&modal)?;

    let Some(Ok(page)) = input.value.as_deref().map(str::parse) else {
        debug!("failed to parse page input `{:?}` as usize", input.value);

        return Ok(());
    };

    let Some(ref msg) = modal.message else {
        warn!("received modal without msg");

        return Ok(());
    };

    let (builder, defer_components) = {
        let mut guard = ctx.paginations.lock(&msg.id).await;

        let Some(pagination) = guard.get_mut() else {
            return Ok(());
        };

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
    };

    respond_modal(defer_components, &modal, &ctx, builder).await
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
