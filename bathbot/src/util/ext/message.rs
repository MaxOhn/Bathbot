use std::{future::IntoFuture, slice};

use bathbot_util::MessageBuilder;
use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use crate::core::Context;

pub trait MessageExt {
    fn update(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> Option<ResponseFuture<Message>>;

    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody>;

    fn reply(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message>;
}

impl MessageExt for (Id<MessageMarker>, Id<ChannelMarker>) {
    fn update(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> Option<ResponseFuture<Message>> {
        let can_view_channel = permissions.map_or(true, |permissions| {
            permissions.contains(Permissions::VIEW_CHANNEL)
        });

        // Lacking permission to edit the message
        if !can_view_channel {
            return None;
        }

        let mut req = ctx.http.update_message(self.1, self.0);

        if let Some(ref content) = builder.content {
            req = req
                .content(Some(content.as_ref()))
                .expect("invalid content");
        }

        let embed = builder.embed.build();
        req = req.embeds(embed.as_option_slice()).expect("invalid embed");

        if let Some(ref components) = builder.components {
            req = req
                .components(Some(components))
                .expect("invalid components");
        }

        Some(req.into_future())
    }

    #[inline]
    fn delete<'l>(&'l self, ctx: &'l Context) -> ResponseFuture<EmptyBody> {
        ctx.http.delete_message(self.1, self.0).into_future()
    }

    fn reply(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        let mut req = ctx.http.create_message(self.1).reply(self.0);

        if let Some(ref content) = builder.content {
            req = req.content(content.as_ref()).expect("invalid content");
        }

        let embed = builder.embed.build();

        if let Some(slice) = embed.as_option_slice() {
            req = req.embeds(slice).expect("invalid embed");
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
}

impl MessageExt for Message {
    #[inline]
    fn update(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> Option<ResponseFuture<Message>> {
        (self.id, self.channel_id).update(ctx, builder, permissions)
    }

    #[inline]
    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody> {
        (self.id, self.channel_id).delete(ctx)
    }

    #[inline]
    fn reply(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        (self.id, self.channel_id).reply(ctx, builder, permissions)
    }
}
