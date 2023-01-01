use std::sync::Arc;

use twilight_model::application::interaction::{Interaction, InteractionData, InteractionType};

use crate::{
    core::Context,
    util::interaction::{InteractionCommand, InteractionComponent, InteractionModal},
};

use self::{
    autocomplete::handle_autocomplete, command::handle_command, component::handle_component,
    modal::handle_modal,
};

mod autocomplete;
mod command;
mod component;
mod modal;

pub async fn handle_interaction(ctx: Arc<Context>, interaction: Interaction) {
    let Interaction {
        app_permissions: permissions,
        channel_id,
        data,
        guild_id,
        id,
        kind,
        member,
        message,
        token,
        user,
        ..
    } = interaction;

    let channel_id = match channel_id {
        Some(channel_id) => channel_id,
        None => return warn!("no channel id for interaction kind {kind:?}"),
    };

    match data {
        Some(InteractionData::ApplicationCommand(data)) => {
            let cmd = InteractionCommand {
                permissions,
                channel_id,
                data,
                guild_id,
                id,
                member,
                token,
                user,
            };

            match kind {
                InteractionType::ApplicationCommand => handle_command(ctx, cmd).await,
                InteractionType::ApplicationCommandAutocomplete => {
                    handle_autocomplete(ctx, cmd).await
                }
                _ => warn!("got unexpected interaction kind {kind:?}"),
            }
        }
        Some(InteractionData::MessageComponent(data)) => {
            let message = match message {
                Some(message) => message,
                None => return warn!("no message in interaction component"),
            };

            let component = InteractionComponent {
                permissions,
                channel_id,
                data,
                guild_id,
                id,
                member,
                message,
                token,
                user,
            };

            handle_component(ctx, component).await
        }
        Some(InteractionData::ModalSubmit(data)) => {
            let modal = InteractionModal {
                permissions,
                channel_id,
                data,
                guild_id,
                id,
                member,
                message,
                token,
                user,
            };

            handle_modal(ctx, modal).await
        }
        None => {}
    }
}
