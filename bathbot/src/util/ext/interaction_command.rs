use std::{borrow::Cow, future::IntoFuture, mem, slice};

use bathbot_util::{EmbedBuilder, MessageBuilder};
use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_interactions::command::CommandInputData;
use twilight_model::{
    application::command::CommandOptionChoice,
    channel::{message::MessageFlags, Message},
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{core::Context, util::interaction::InteractionCommand};

pub trait InteractionCommandExt {
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

    /// Callback to an autocomplete action.
    fn autocomplete(
        &self,
        ctx: &Context,
        choices: Vec<CommandOptionChoice>,
    ) -> ResponseFuture<EmptyBody>;
}

impl InteractionCommandExt for InteractionCommand {
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
            content: builder.content.map(|c| c.into_owned()),
            embeds: builder.embed.map(|e| vec![e]),
            flags: ephemeral.then_some(MessageFlags::EPHEMERAL),
            attachments,
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    #[inline]
    fn defer(&self, ctx: &Context, ephemeral: bool) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            flags: ephemeral.then_some(MessageFlags::EPHEMERAL),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    #[inline]
    fn update<'l>(
        &'l self,
        ctx: &'l Context,
        builder: &'l MessageBuilder<'l>,
    ) -> ResponseFuture<Message> {
        let client = ctx.interaction();

        let mut req = client.update_response(&self.token);

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

        if let Some(attachment) = builder.attachment.as_ref().filter(|_| {
            self.permissions.map_or(true, |permissions| {
                permissions.contains(Permissions::ATTACH_FILES)
            })
        }) {
            req = req.attachments(slice::from_ref(attachment)).unwrap();
        }

        req.into_future()
    }

    #[inline]
    fn error(&self, ctx: &Context, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().description(content).color_red().build();

        ctx.interaction()
            .update_response(&self.token)
            .embeds(Some(&[embed]))
            .expect("invalid embed")
            .into_future()
    }

    #[inline]
    fn error_callback(
        &self,
        ctx: &Context,
        content: impl Into<String>,
    ) -> ResponseFuture<EmptyBody> {
        let embed = EmbedBuilder::new().description(content).color_red().build();

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
            .into_future()
    }

    #[inline]
    fn autocomplete(
        &self,
        ctx: &Context,
        choices: Vec<CommandOptionChoice>,
    ) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
            data: Some(data),
        };

        ctx.interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }
}
