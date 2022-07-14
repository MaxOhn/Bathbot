use std::slice;

use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use crate::{core::Context, util::builder::MessageBuilder};

pub trait MessageExt {
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message>;

    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody>;
}

impl MessageExt for (Id<MessageMarker>, Id<ChannelMarker>) {
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message> {
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

        req.exec()
    }

    #[inline]
    fn delete<'l>(&'l self, ctx: &'l Context) -> ResponseFuture<EmptyBody> {
        ctx.http.delete_message(self.1, self.0).exec()
    }
}

impl MessageExt for Message {
    #[inline]
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message> {
        (self.id, self.channel_id).update(ctx, builder)
    }

    #[inline]
    fn delete(&self, ctx: &Context) -> ResponseFuture<EmptyBody> {
        (self.id, self.channel_id).delete(ctx)
    }
}
