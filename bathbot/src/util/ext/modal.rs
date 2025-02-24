use std::{future::IntoFuture, slice};

use bathbot_util::MessageBuilder;
use twilight_http::response::{ResponseFuture, marker::EmptyBody};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{core::Context, util::interaction::InteractionModal};

pub trait ModalExt {
    /// Ackowledge the modal and respond immediatly by updating the message.
    fn callback(&self, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the modal but don't respond yet.
    fn defer(&self) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the modal either via
    /// [`ModalExt::callback`] or [`ModalExt::defer`],
    /// use this to update the message.
    fn update(&self, builder: MessageBuilder<'_>) -> ResponseFuture<Message>;
}

impl ModalExt for InteractionModal {
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
}
