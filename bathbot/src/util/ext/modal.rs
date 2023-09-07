use std::future::IntoFuture;

use bathbot_util::MessageBuilder;
use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use super::MessageExt;
use crate::{core::Context, util::interaction::InteractionModal};

pub trait ModalExt {
    /// Ackowledge the modal and respond immediatly by updating the message.
    fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the modal but don't respond yet.
    fn defer(&self, ctx: &Context) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the modal either via
    /// [`ModalExt::callback`] or [`ModalExt::defer`],
    /// use this to update the message.
    ///
    /// Note: Can only be used if `ModalSubmitInteraction::message` is `Some`.
    fn update(&self, ctx: &Context, builder: MessageBuilder<'_>)
        -> Option<ResponseFuture<Message>>;
}

impl ModalExt for InteractionModal {
    #[inline]
    fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody> {
        let attachments = builder
            .attachment
            .filter(|_| {
                self.permissions.map_or(true, |permissions| {
                    permissions.contains(Permissions::ATTACH_FILES)
                })
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

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    #[inline]
    fn defer(&self, ctx: &Context) -> ResponseFuture<EmptyBody> {
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    #[inline]
    fn update(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
    ) -> Option<ResponseFuture<Message>> {
        self.message
            .as_ref()
            .expect("no message in modal")
            .update(ctx, builder, self.permissions)
    }
}
