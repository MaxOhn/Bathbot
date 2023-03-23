use std::{borrow::Cow, future::IntoFuture};

use bathbot_util::{modal::ModalBuilder, MessageBuilder};
use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{core::Context, util::interaction::InteractionComponent};

use super::MessageExt;

pub trait ComponentExt {
    /// Ackowledge the component and respond immediatly by updating the message.
    fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the component but don't respond yet.
    fn defer(&self, ctx: &Context) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the component either via
    /// [`ComponentExt::callback`] or [`ComponentExt::defer`],
    /// use this to update the message.
    fn update(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
    ) -> Option<ResponseFuture<Message>>;

    /// Acknowledge a component by responding with a modal.
    fn modal(&self, ctx: &Context, modal: ModalBuilder) -> ResponseFuture<EmptyBody>;
}

impl ComponentExt for InteractionComponent {
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
            embeds: builder.embed.map(|e| vec![e]),
            content: builder.content.map(Cow::into_owned),
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
        builder: &MessageBuilder<'_>,
    ) -> Option<ResponseFuture<Message>> {
        self.message.update(ctx, builder, self.permissions)
    }

    #[inline]
    fn modal(&self, ctx: &Context, modal: ModalBuilder) -> ResponseFuture<EmptyBody> {
        let response = InteractionResponse {
            kind: InteractionResponseType::Modal,
            data: Some(modal.build()),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }
}
