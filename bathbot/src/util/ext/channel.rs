use std::{future::IntoFuture, slice};

use bathbot_util::{EmbedBuilder, MessageBuilder};
use twilight_http::response::ResponseFuture;
use twilight_model::{
    channel::Message,
    guild::Permissions,
    id::{marker::ChannelMarker, Id},
};

use crate::core::Context;

pub trait ChannelExt {
    /// Create a message inside a green embed
    fn create_message(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message>;

    /// Create a message inside a red embed
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message>;

    /// Create a message without embed; only content
    fn plain_message(&self, ctx: &Context, content: &str) -> ResponseFuture<Message>;
}

impl ChannelExt for Id<ChannelMarker> {
    fn create_message(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        let mut req = ctx.http.create_message(*self);

        if let Some(ref content) = builder.content {
            req = req.content(content.as_ref()).expect("invalid content");
        }

        if let Some(ref embed) = builder.embed {
            req = req.embeds(slice::from_ref(embed)).expect("invalid embed");
        }

        if let Some(components) = builder.components.as_deref() {
            req = req.components(components).expect("invalid components");
        }

        match builder.attachment.as_ref().filter(|_| {
            permissions.map_or(true, |permissions| {
                permissions.contains(Permissions::ATTACH_FILES)
            })
        }) {
            Some(attachment) => req
                .attachments(slice::from_ref(attachment))
                .unwrap()
                .into_future(),
            None => req.into_future(),
        }
    }

    #[inline]
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().color_red().description(content).build();

        ctx.http
            .create_message(*self)
            .embeds(&[embed])
            .expect("invalid embed")
            .into_future()
    }

    #[inline]
    fn plain_message(&self, ctx: &Context, content: &str) -> ResponseFuture<Message> {
        ctx.http
            .create_message(*self)
            .content(content)
            .expect("invalid content")
            .into_future()
    }
}

impl ChannelExt for Message {
    #[inline]
    fn create_message(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        self.channel_id.create_message(ctx, builder, permissions)
    }

    #[inline]
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message> {
        self.channel_id.error(ctx, content)
    }

    #[inline]
    fn plain_message(&self, ctx: &Context, content: &str) -> ResponseFuture<Message> {
        self.channel_id.plain_message(ctx, content)
    }
}
