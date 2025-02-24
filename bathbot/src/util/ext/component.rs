use std::{borrow::Cow, future::IntoFuture, slice};

use bathbot_util::{MessageBuilder, modal::ModalBuilder};
use twilight_http::response::{ResponseFuture, marker::EmptyBody};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{core::Context, util::interaction::InteractionComponent};

pub trait ComponentExt {
    /// Ackowledge the component and respond immediatly by updating the message.
    fn callback(&self, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the component but don't respond yet.
    fn defer(&self) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the component either via
    /// [`ComponentExt::callback`] or [`ComponentExt::defer`],
    /// use this to update the message.
    fn update(&self, builder: MessageBuilder<'_>) -> ResponseFuture<Message>;

    /// Acknowledge a component by responding with a modal.
    fn modal(&self, modal: ModalBuilder) -> ResponseFuture<EmptyBody>;
}

impl ComponentExt for InteractionComponent {
    fn callback(&self, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody> {
        let attachments = builder
            .attachment
            .filter(|_| {
                self.permissions
                    .is_none_or(|permissions| permissions.contains(Permissions::ATTACH_FILES))
            })
            .map(|attachment| vec![attachment]);

        let data = InteractionResponseData {
            components: builder.components,
            embeds: builder.embed.into(),
            content: builder.content.map(Cow::into_owned),
            attachments,
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::UpdateMessage,
            data: Some(data),
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    fn defer(&self) -> ResponseFuture<EmptyBody> {
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    fn update(&self, builder: MessageBuilder<'_>) -> ResponseFuture<Message> {
        let client = Context::interaction();

        let mut req = client.update_response(&self.token);

        if let Some(ref content) = builder.content {
            req = req.content(Some(content.as_ref()));
        }

        let embed = builder.embed.build();
        req = req.embeds(embed.as_option_slice());

        if let Some(ref components) = builder.components {
            req = req.components(Some(components));
        }

        if let Some(attachment) = builder.attachment.as_ref().filter(|_| {
            self.permissions
                .is_none_or(|permissions| permissions.contains(Permissions::ATTACH_FILES))
        }) {
            req = req.attachments(slice::from_ref(attachment));
        }

        req.into_future()
    }

    fn modal(&self, modal: ModalBuilder) -> ResponseFuture<EmptyBody> {
        let response = InteractionResponse {
            kind: InteractionResponseType::Modal,
            data: Some(modal.build()),
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }
}
