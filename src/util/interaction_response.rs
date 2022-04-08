use twilight_model::{
    application::{command::CommandOptionChoice, component::Component},
    channel::{embed::Embed, message::MessageFlags},
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use super::MessageBuilder;

pub struct InteractionResponseBuilder;

impl InteractionResponseBuilder {
    /// Responds to an interaction with a message.
    pub fn msg(builder: MessageBuilder<'_>, flags: Option<MessageFlags>) -> InteractionResponse {
        let data = InteractionResponseData {
            content: builder.content.map(|c| c.into_owned()),
            embeds: builder.embed.map(|e| vec![e]),
            flags,
            ..Default::default()
        };

        InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        }
    }

    /// Acknowledges an interaction, showing a loading state, and allowing for the message to be edited later.
    pub fn deferred_msg(flags: Option<MessageFlags>) -> InteractionResponse {
        let data = InteractionResponseData {
            flags,
            ..Default::default()
        };

        InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: Some(data),
        }
    }

    /// Acknowledges a component interaction and edits the message.
    ///
    /// This is only valid for components.
    pub fn update(builder: MessageBuilder<'_>) -> InteractionResponse {
        let data = InteractionResponseData {
            components: builder.components,
            embeds: builder.embed.map(|e| vec![e]),
            ..Default::default()
        };

        InteractionResponse {
            kind: InteractionResponseType::UpdateMessage,
            data: Some(data),
        }
    }

    /// Acknowledges a component interaction, allowing for the message to be edited later.
    ///
    /// This is only valid for components.
    pub fn deferred_update() -> InteractionResponse {
        InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        }
    }

    /// Respond to an autocomplete interaction with suggested choices.
    pub fn autocomplete(choices: Vec<CommandOptionChoice>) -> InteractionResponse {
        let data = InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        };

        InteractionResponse {
            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
            data: Some(data),
        }
    }
}
