use std::{borrow::Cow, mem, slice};

use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_interactions::command::CommandInputData;
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::{message::MessageFlags, Message},
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{
    core::Context,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::RED,
    },
};

pub trait ApplicationCommandExt {
    /// Extract input data containing options and resolved values
    fn input_data(&mut self) -> CommandInputData<'static>;

    /// Ackowledge the command and respond immediatly.
    fn callback(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        ephemeral: bool,
    ) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the command but don't respond yet.
    ///
    /// Must use [`ApplicationCommandExt::update`] afterwards!
    fn defer(&self, ctx: &Context, ephemeral: bool) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the command either via
    /// [`ApplicationCommandExt::callback`] or [`ApplicationCommandExt::defer`],
    /// use this to update the response.
    fn update(&self, ctx: &Context, builder: &MessageBuilder<'_>) -> ResponseFuture<Message>;

    /// Update a command to some content in a red embed.
    ///
    /// Be sure the command was deferred beforehand.
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message>;

    /// Respond to a command with some content in a red embed.
    ///
    /// Be sure the command was **not** deferred beforehand.
    fn error_callback(
        &self,
        ctx: &Context,
        content: impl Into<String>,
    ) -> ResponseFuture<EmptyBody>;
}

impl ApplicationCommandExt for ApplicationCommand {
    #[inline]
    fn input_data(&mut self) -> CommandInputData<'static> {
        CommandInputData {
            options: mem::take(&mut self.data.options),
            resolved: self.data.resolved.take().map(Cow::Owned),
        }
    }

    #[inline]
    fn callback(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        ephemeral: bool,
    ) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            components: builder.components,
            content: builder.content.map(|c| c.into_owned()),
            embeds: builder.embed.map(|e| vec![e]),
            flags: ephemeral.then(|| MessageFlags::EPHEMERAL),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }

    #[inline]
    fn defer(&self, ctx: &Context, ephemeral: bool) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            flags: ephemeral.then(|| MessageFlags::EPHEMERAL),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }

    #[inline]
    fn update<'l>(
        &'l self,
        ctx: &'l Context,
        builder: &'l MessageBuilder<'l>,
    ) -> ResponseFuture<Message> {
        let client = ctx.interaction();

        let mut req = client
            .update_response(&self.token)
            .content(builder.content.as_ref().map(Cow::as_ref))
            .expect("invalid content")
            .embeds(builder.embed.as_ref().map(slice::from_ref))
            .expect("invalid embed")
            .components(builder.components.as_deref())
            .expect("invalid components");

        if let Some(ref attachment) = builder.attachment {
            req = req.attachments(slice::from_ref(attachment)).unwrap();
        }

        req.exec()
    }

    #[inline]
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().description(content).color(RED).build();

        ctx.interaction()
            .update_response(&self.token)
            .embeds(Some(&[embed]))
            .expect("invalid embed")
            .exec()
    }

    #[inline]
    fn error_callback(
        &self,
        ctx: &Context,
        content: impl Into<String>,
    ) -> ResponseFuture<EmptyBody> {
        let embed = EmbedBuilder::new().description(content).color(RED).build();

        let data = InteractionResponseData {
            embeds: Some(vec![embed]),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .exec()
    }
}
