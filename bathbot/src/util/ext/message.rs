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
        builder: &MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> Option<ResponseFuture<Message>>;

    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody>;
}

impl MessageExt for (Id<MessageMarker>, Id<ChannelMarker>) {
    fn update(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
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

        if let Some(ref embed) = builder.embed {
            req = req
                .embeds(Some(slice::from_ref(embed)))
                .expect("invalid embed");
        }

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
}

impl MessageExt for Message {
    #[inline]
    fn update(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
        permissions: Option<Permissions>,
    ) -> Option<ResponseFuture<Message>> {
        (self.id, self.channel_id).update(ctx, builder, permissions)
    }

    #[inline]
    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody> {
        (self.id, self.channel_id).delete(ctx)
    }
}
