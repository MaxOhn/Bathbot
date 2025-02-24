use std::{future::IntoFuture, slice};

use bathbot_util::{EmbedBuilder, MessageBuilder};
use twilight_http::response::ResponseFuture;
use twilight_model::{
    channel::Message,
    guild::Permissions,
    id::{Id, marker::ChannelMarker},
};

use crate::core::Context;

pub trait ChannelExt {
    /// Create a message inside a green embed
    fn create_message(
        &self,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message>;

    /// Create a message inside a red embed
    fn error(&self, content: impl Into<String>) -> ResponseFuture<Message>;

    /// Create a message without embed; only content
    fn plain_message(&self, content: &str) -> ResponseFuture<Message>;
}

impl ChannelExt for Id<ChannelMarker> {
    fn create_message(
        &self,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        let mut req = Context::http().create_message(*self);

        if let Some(ref content) = builder.content {
            req = req.content(content.as_ref());
        }

        let embed = builder.embed.build();

        if let Some(slice) = embed.as_option_slice() {
            req = req.embeds(slice);
        }

        if let Some(components) = builder.components.as_deref() {
            req = req.components(components);
        }

        match builder.attachment.as_ref().filter(|_| {
            permissions.is_none_or(|permissions| permissions.contains(Permissions::ATTACH_FILES))
        }) {
            Some(attachment) => req.attachments(slice::from_ref(attachment)).into_future(),
            None => req.into_future(),
        }
    }

    fn error(&self, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().color_red().description(content).build();

        Context::http()
            .create_message(*self)
            .embeds(&[embed])
            .into_future()
    }

    fn plain_message(&self, content: &str) -> ResponseFuture<Message> {
        Context::http()
            .create_message(*self)
            .content(content)
            .into_future()
    }
}

impl ChannelExt for Message {
    fn create_message(
        &self,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        self.channel_id.create_message(builder, permissions)
    }

    fn error(&self, content: impl Into<String>) -> ResponseFuture<Message> {
        self.channel_id.error(content)
    }

    fn plain_message(&self, content: &str) -> ResponseFuture<Message> {
        self.channel_id.plain_message(content)
    }
}
