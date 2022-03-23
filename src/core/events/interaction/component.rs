use std::{mem, sync::Arc};

use eyre::Report;
use twilight_model::application::interaction::MessageComponentInteraction;

use crate::{
    commands::{
        fun::{handle_higher, handle_lower},
        help::handle_help_component,
    },
    core::{events::log_command, Context},
    games::bg::components::*,
};

pub async fn handle_component(ctx: Arc<Context>, mut component: Box<MessageComponentInteraction>) {
    let name = mem::take(&mut component.data.custom_id);
    log_command(&ctx, &*component, &name);
    ctx.stats.increment_component(&name);

    let res = match name.as_str() {
        "help_menu" | "help_back" => handle_help_component(&ctx, component).await,
        "bg_start_include" => handle_bg_start_include(&ctx, component).await,
        "bg_start_exclude" => handle_bg_start_exclude(&ctx, component).await,
        "bg_start_effects" => handle_bg_start_effects(&ctx, component).await,
        "bg_start_button" => handle_bg_start_button(ctx, component).await,
        "bg_start_cancel" => handle_bg_start_cancel(&ctx, component).await,
        "higher_button" => handle_higher(ctx, *component).await,
        "lower_button" => handle_lower(ctx, *component).await,
        _ => return error!("unknown message component `{name}`"),
    };

    if let Err(err) = res {
        let wrap = format!("failed to process component `{name}`");
        error!("{:?}", Report::new(err).wrap_err(wrap));
    }
}
