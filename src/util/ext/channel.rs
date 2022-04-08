use std::slice;

use twilight_http::response::ResponseFuture;
use twilight_model::{
    channel::Message,
    id::{marker::ChannelMarker, Id},
};

use crate::{
    core::Context,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::RED,
    },
};

pub trait ChannelExt {
    /// Create a message inside a green embed
    fn create_message(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
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

        match builder.attachment {
            Some(ref attachment) => req.attachments(slice::from_ref(attachment)).unwrap().exec(),
            None => req.exec(),
        }
    }

    #[inline]
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        ctx.http
            .create_message(*self)
            .embeds(&[embed])
            .expect("invalid embed")
            .exec()
    }

    #[inline]
    fn plain_message(&self, ctx: &Context, content: &str) -> ResponseFuture<Message> {
        ctx.http
            .create_message(*self)
            .content(content)
            .expect("invalid content")
            .exec()
    }
}

impl ChannelExt for Message {
    #[inline]
    fn create_message(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
    ) -> ResponseFuture<Message> {
        self.channel_id.create_message(ctx, builder)
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
