use std::sync::Arc;

use twilight_model::application::interaction::Interaction;

use crate::core::Context;

use self::{
    autocomplete::handle_autocomplete, command::handle_command, component::handle_component,
};

mod autocomplete;
mod command;
mod component;

pub async fn handle_interaction(ctx: Arc<Context>, interaction: Interaction) {
    match interaction {
        Interaction::ApplicationCommand(cmd) => handle_command(ctx, cmd).await,
        Interaction::MessageComponent(component) => handle_component(ctx, component).await,
        Interaction::ApplicationCommandAutocomplete(cmd) => handle_autocomplete(ctx, cmd).await,
        _ => {}
    }
}
