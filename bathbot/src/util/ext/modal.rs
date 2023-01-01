use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    channel::Message,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{
    core::Context,
    util::{builder::MessageBuilder, interaction::InteractionModal},
};

use super::MessageExt;

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
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message>;
}

impl ModalExt for InteractionModal {
    #[inline]
    fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            components: builder.components,
            embeds: builder.embed.map(|e| vec![e]),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::UpdateMessage,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }

    #[inline]
    fn defer(&self, ctx: &Context) -> ResponseFuture<EmptyBody> {
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }

    #[inline]
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message> {
        self.message
            .as_ref()
            .expect("no message in modal")
            .update(ctx, builder)
    }
}
