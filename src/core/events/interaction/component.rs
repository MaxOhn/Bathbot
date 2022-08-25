use std::{mem, sync::Arc};

use eyre::Report;

use crate::{
    commands::help::handle_help_component,
    core::{events::log_command, Context},
    games::{bg::components::*, hl::components::*},
    pagination::components::*,
    util::interaction::InteractionComponent,
};

pub async fn handle_component(ctx: Arc<Context>, mut component: InteractionComponent) {
    let name = mem::take(&mut component.data.custom_id);
    log_command(&ctx, &component, &name);
    ctx.stats.increment_component(&name);

    let res = match name.as_str() {
        "help_menu" | "help_back" => handle_help_component(&ctx, component).await,
        "bg_start_include" => handle_bg_start_include(&ctx, component).await,
        "bg_start_exclude" => handle_bg_start_exclude(&ctx, component).await,
        "bg_start_effects" => handle_bg_start_effects(&ctx, component).await,
        "bg_start_button" => handle_bg_start_button(ctx, component).await,
        "bg_start_cancel" => handle_bg_start_cancel(&ctx, component).await,
        "higher_button" => handle_higher(ctx, component).await,
        "lower_button" => handle_lower(ctx, component).await,
        "try_again_button" => handle_try_again(ctx, component).await,
        "next_higherlower" => handle_next_higherlower(ctx, component).await,
        "pagination_start" => handle_pagination_start(ctx, component).await,
        "pagination_back" => handle_pagination_back(ctx, component).await,
        "pagination_custom" => handle_pagination_custom(ctx, component).await,
        "pagination_step" => handle_pagination_step(ctx, component).await,
        "pagination_end" => handle_pagination_end(ctx, component).await,
        "profile_compact" => handle_profile_compact(ctx, component).await,
        "profile_medium" => handle_profile_medium(ctx, component).await,
        "profile_full" => handle_profile_full(ctx, component).await,
        _ => return error!("unknown message component `{name}`"),
    };

    if let Err(err) = res {
        let wrap = format!("failed to process component `{name}`");
        error!("{:?}", Report::new(err).wrap_err(wrap));
    }
}
