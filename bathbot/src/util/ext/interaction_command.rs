use std::{borrow::Cow, future::IntoFuture, mem, slice};

use bathbot_util::{EmbedBuilder, MessageBuilder};
use twilight_http::response::{marker::EmptyBody, ResponseFuture};
use twilight_interactions::command::CommandInputData;
use twilight_model::{
    application::{
        command::{CommandOptionChoice, CommandType},
        interaction::application_command::CommandOptionValue,
    },
    channel::{message::MessageFlags, Message},
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{
    core::Context,
    util::{interaction::InteractionCommand, CheckPermissions},
};

pub trait InteractionCommandExt {
    /// Extract input data containing options and resolved values
    fn input_data(&mut self) -> CommandInputData<'static>;

    /// Returns the command's subcommand group and subcommand.
    ///
    /// If either is not present, their name will be an empty string.
    /// If the method was not called on a slash commands, `None` is returned.
    fn group_sub(&self) -> Option<(Cow<'static, str>, Cow<'static, str>)>;

    /// Ackowledge the command and respond immediatly.
    fn callback(&self, builder: MessageBuilder<'_>, ephemeral: bool) -> ResponseFuture<EmptyBody>;

    /// Ackownledge the command but don't respond yet.
    ///
    /// Must use [`InteractionCommandExt::update`] afterwards!
    fn defer(&self, ephemeral: bool) -> ResponseFuture<EmptyBody>;

    /// After having already ackowledged the command either via
    /// [`InteractionCommandExt::callback`] or [`InteractionCommandExt::defer`],
    /// use this to update the response.
    fn update(&self, builder: MessageBuilder<'_>) -> ResponseFuture<Message>;

    /// Update a command to some content in a red embed.
    ///
    /// Be sure the command was deferred beforehand.
    fn error(&self, content: impl Into<String>) -> ResponseFuture<Message> {
        let embed = EmbedBuilder::new().description(content).color_red();
        let builder = MessageBuilder::new().embed(embed);

        self.update(builder)
    }

    /// Respond to a command with some content in a red embed.
    ///
    /// Be sure the command was **not** deferred beforehand.
    fn error_callback(&self, content: impl Into<String>) -> ResponseFuture<EmptyBody> {
        let embed = EmbedBuilder::new().description(content).color_red();
        let builder = MessageBuilder::new().embed(embed);

        self.callback(builder, false)
    }

    /// Callback to an autocomplete action.
    fn autocomplete(&self, choices: Vec<CommandOptionChoice>) -> ResponseFuture<EmptyBody>;
}

impl InteractionCommandExt for InteractionCommand {
    fn input_data(&mut self) -> CommandInputData<'static> {
        CommandInputData {
            options: mem::take(&mut self.data.options),
            resolved: self.data.resolved.take().map(Cow::Owned),
        }
    }

    fn group_sub(&self) -> Option<(Cow<'static, str>, Cow<'static, str>)> {
        if self.data.kind != CommandType::ChatInput {
            return None;
        }

        let Some(option) = self.data.options.first() else {
            return Some(Default::default());
        };

        let group_sub = match option.value {
            CommandOptionValue::SubCommand(_) => (Default::default(), option.name.clone().into()),
            CommandOptionValue::SubCommandGroup(ref vec) => {
                let group = option.name.clone().into();

                let sub = match vec.first() {
                    Some(sub) => sub.name.clone().into(),
                    None => Default::default(),
                };

                (group, sub)
            }
            _ => Default::default(),
        };

        Some(group_sub)
    }

    fn callback(&self, builder: MessageBuilder<'_>, ephemeral: bool) -> ResponseFuture<EmptyBody> {
        let attachments = builder
            .attachment
            .filter(|_| self.can_attach_file())
            .map(|attachment| vec![attachment]);

        let data = InteractionResponseData {
            components: builder.components,
            content: builder.content.map(Cow::into_owned),
            embeds: builder.embed.into(),
            flags: ephemeral.then_some(MessageFlags::EPHEMERAL),
            attachments,
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    fn defer(&self, ephemeral: bool) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            flags: ephemeral.then_some(MessageFlags::EPHEMERAL),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: Some(data),
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }

    fn update<'l>(&'l self, builder: MessageBuilder<'l>) -> ResponseFuture<Message> {
        InteractionToken(&self.token).update(builder, self.permissions)
    }

    fn autocomplete(&self, choices: Vec<CommandOptionChoice>) -> ResponseFuture<EmptyBody> {
        let data = InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
            data: Some(data),
        };

        Context::interaction()
            .create_response(self.id, &self.token, &response)
            .into_future()
    }
}

pub struct InteractionToken<'a>(pub &'a str);

impl InteractionToken<'_> {
    pub fn update<'l>(
        &'l self,
        builder: MessageBuilder<'l>,
        permissions: Option<Permissions>,
    ) -> ResponseFuture<Message> {
        let client = Context::interaction();

        let mut req = client.update_response(self.0);

        if let Some(ref content) = builder.content {
            req = req.content(Some(content.as_ref()));
        }

        let embed = builder.embed.build();

        if let Some(embeds) = embed.as_option_slice() {
            req = req.embeds(Some(embeds));
        }

        if let Some(ref components) = builder.components {
            req = req.components(Some(components));
        }

        if let Some(attachment) = builder.attachment.as_ref().filter(|_| {
            permissions.is_none_or(|permissions| permissions.contains(Permissions::ATTACH_FILES))
        }) {
            req = req.attachments(slice::from_ref(attachment));
        }

        req.into_future()
    }
}
