use bathbot_util::MessageBuilder;
use eyre::{ContextCompat, Result, WrapErr};
use twilight_http::Response;
use twilight_model::{
    channel::Message,
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::Context,
    util::{
        interaction::InteractionCommand, Authored, ChannelExt, InteractionCommandExt, MessageExt,
    },
};

pub enum CommandOrigin<'d> {
    Message {
        msg: &'d Message,
        permissions: Option<Permissions>,
    },
    Interaction {
        command: &'d mut InteractionCommand,
    },
}

impl CommandOrigin<'_> {
    pub fn user_id(&self) -> Result<Id<UserMarker>> {
        match self {
            CommandOrigin::Message { msg, .. } => Ok(msg.author.id),
            CommandOrigin::Interaction { command } => command.user_id(),
        }
    }

    pub fn channel_id(&self) -> Id<ChannelMarker> {
        match self {
            CommandOrigin::Message { msg, .. } => msg.channel_id,
            CommandOrigin::Interaction { command } => command.channel_id,
        }
    }

    pub fn guild_id(&self) -> Option<Id<GuildMarker>> {
        match self {
            CommandOrigin::Message { msg, .. } => msg.guild_id,
            CommandOrigin::Interaction { command } => command.guild_id,
        }
    }

    /// Respond to something.
    ///
    /// In case of a message, discard the response message created.
    ///
    /// In case of an interaction, the response will **not** be ephemeral.
    pub async fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> Result<()> {
        match self {
            Self::Message { msg, permissions } => msg
                .create_message(ctx, builder, *permissions)
                .await
                .map(|_| ())
                .wrap_err("failed to create message to callback"),
            Self::Interaction { command } => command
                .callback(ctx, builder, false)
                .await
                .map(|_| ())
                .wrap_err("failed to callback"),
        }
    }

    /// Respond to something and return the resulting response message.
    ///
    /// In case of an interaction, the response will **not** be ephemeral.
    pub async fn callback_with_response(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
    ) -> Result<Response<Message>> {
        match self {
            Self::Message { msg, permissions } => msg
                .create_message(ctx, builder, *permissions)
                .await
                .wrap_err("failed to create message for response callback"),
            Self::Interaction { command } => {
                command
                    .callback(ctx, builder, false)
                    .await
                    .wrap_err("failed to callback for response")?;

                ctx.interaction()
                    .response(&command.token)
                    .await
                    .wrap_err("failed to get response message")
            }
        }
    }

    #[allow(unused)]
    /// Respond to something.
    ///
    /// In case of a message, ignore the flags and discard the response message
    /// created.
    pub async fn callback_with_flags(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        ephemeral: bool,
    ) -> Result<()> {
        match self {
            Self::Message { msg, permissions } => msg
                .create_message(ctx, builder, *permissions)
                .await
                .map(|_| ())
                .wrap_err("failed to create message for flagged callback"),
            Self::Interaction { command } => command
                .callback(ctx, builder, ephemeral)
                .await
                .map(|_| ())
                .wrap_err("failed to callback with flags"),
        }
    }

    /// Respond to something and return the resulting response message.
    ///
    /// In case of an interaction, be sure you already called back the invoke,
    /// either through deferring or a previous initial response.
    /// Also be sure this is only called once.
    /// Afterwards, use the resulting response message instead.
    pub async fn create_message(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
    ) -> Result<Response<Message>> {
        match self {
            Self::Message { msg, permissions } => msg
                .create_message(ctx, builder, *permissions)
                .await
                .wrap_err("failed to create message as response"),
            Self::Interaction { command } => command
                .update(ctx, builder)
                .await
                .wrap_err("failed to update as response"),
        }
    }

    /// Update a response and return the resulting response message.
    ///
    /// In case of an interaction, be sure this is the first and only time you
    /// call this. Afterwards, you must update the resulting message.
    pub async fn update(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
    ) -> Result<Response<Message>> {
        match self {
            Self::Message { msg, permissions } => msg
                .update(ctx, builder, *permissions)
                .wrap_err("lacking permission to update message")?
                .await
                .wrap_err("failed to update message"),
            Self::Interaction { command } => command
                .update(ctx, builder)
                .await
                .wrap_err("failed to update interaction message"),
        }
    }

    /// Respond with a red embed.
    ///
    /// In case of an interaction, be sure you already called back beforehand.
    pub async fn error(&self, ctx: &Context, content: impl Into<String>) -> Result<()> {
        match self {
            Self::Message { msg, .. } => msg
                .error(ctx, content)
                .await
                .map(|_| ())
                .wrap_err("failed to respond with error"),
            Self::Interaction { command } => command
                .error(ctx, content)
                .await
                .map(|_| ())
                .wrap_err("failed to respond with error"),
        }
    }

    /// Respond with a red embed.
    ///
    /// In case of an interaction, be sure this is the first and only time you
    /// call this. The response will not be ephemeral.
    pub async fn error_callback(&self, ctx: &Context, content: impl Into<String>) -> Result<()> {
        match self {
            CommandOrigin::Message { msg, .. } => msg
                .error(ctx, content)
                .await
                .map(|_| ())
                .wrap_err("failed to callback with error"),
            CommandOrigin::Interaction { command } => command
                .error_callback(ctx, content)
                .await
                .map(|_| ())
                .wrap_err("failed to callback with error"),
        }
    }
}

impl<'d> CommandOrigin<'d> {
    pub fn from_msg(msg: &'d Message, permissions: Option<Permissions>) -> Self {
        Self::Message { msg, permissions }
    }

    pub fn from_interaction(command: &'d mut InteractionCommand) -> Self {
        Self::Interaction { command }
    }
}

impl<'d> From<&'d Message> for CommandOrigin<'d> {
    #[inline]
    fn from(msg: &'d Message) -> Self {
        Self::from_msg(msg, None)
    }
}

impl<'d> From<&'d mut InteractionCommand> for CommandOrigin<'d> {
    #[inline]
    fn from(command: &'d mut InteractionCommand) -> Self {
        Self::from_interaction(command)
    }
}
