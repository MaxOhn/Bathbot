use twilight_http::{Error as HttpError, Response};
use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::Context,
    error::Error,
    util::{
        builder::MessageBuilder, interaction::InteractionCommand, Authored, ChannelExt,
        InteractionCommandExt, MessageExt,
    },
    BotResult,
};

type HttpResult<T> = Result<T, HttpError>;

pub enum CommandOrigin<'d> {
    Message { msg: &'d Message },
    Interaction { command: &'d mut InteractionCommand },
}

impl CommandOrigin<'_> {
    pub fn user_id(&self) -> BotResult<Id<UserMarker>> {
        match self {
            CommandOrigin::Message { msg } => Ok(msg.author.id),
            CommandOrigin::Interaction { command } => command.user_id(),
        }
    }

    pub fn channel_id(&self) -> Id<ChannelMarker> {
        match self {
            CommandOrigin::Message { msg } => msg.channel_id,
            CommandOrigin::Interaction { command } => command.channel_id,
        }
    }

    pub fn guild_id(&self) -> Option<Id<GuildMarker>> {
        match self {
            CommandOrigin::Message { msg } => msg.guild_id,
            CommandOrigin::Interaction { command } => command.guild_id,
        }
    }

    /// Respond to something.
    ///
    /// In case of a message, discard the response message created.
    ///
    /// In case of an interaction, the response will **not** be ephemeral.
    pub async fn callback(&self, ctx: &Context, builder: MessageBuilder<'_>) -> HttpResult<()> {
        match self {
            Self::Message { msg } => msg.create_message(ctx, &builder).await.map(|_| ()),
            Self::Interaction { command } => {
                command.callback(ctx, builder, false).await.map(|_| ())
            }
        }
    }

    /// Respond to something and return the resulting response message.
    ///
    /// In case of an interaction, the response will **not** be ephemeral.
    pub async fn callback_with_response(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
    ) -> HttpResult<Response<Message>> {
        match self {
            Self::Message { msg } => msg.create_message(ctx, &builder).await,
            Self::Interaction { command } => {
                command.callback(ctx, builder, false).await?;

                ctx.interaction().response(&command.token).exec().await
            }
        }
    }

    #[allow(unused)]
    /// Respond to something.
    ///
    /// In case of a message, ignore the flags and discard the response message created.
    pub async fn callback_with_flags(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'_>,
        ephemeral: bool,
    ) -> HttpResult<()> {
        match self {
            Self::Message { msg } => msg.create_message(ctx, &builder).await.map(|_| ()),
            Self::Interaction { command } => {
                command.callback(ctx, builder, ephemeral).await.map(|_| ())
            }
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
        builder: &MessageBuilder<'_>,
    ) -> HttpResult<Response<Message>> {
        match self {
            Self::Message { msg } => msg.create_message(ctx, builder).await,
            Self::Interaction { command } => command.update(ctx, builder).await,
        }
    }

    #[allow(unused)]
    /// Update a response and return the resulting response message.
    ///
    /// In case of an interaction, be sure this is the first and only time you call this.
    /// Afterwards, you must update the resulting message.
    pub async fn update(
        &self,
        ctx: &Context,
        builder: &MessageBuilder<'_>,
    ) -> HttpResult<Response<Message>> {
        match self {
            Self::Message { msg } => msg.update(ctx, builder).await,
            Self::Interaction { command } => command.update(ctx, builder).await,
        }
    }

    /// Respond with a red embed.
    ///
    /// In case of an interaction, be sure you already called back beforehand.
    pub async fn error(&self, ctx: &Context, content: impl Into<String>) -> BotResult<()> {
        match self {
            Self::Message { msg } => msg
                .error(ctx, content)
                .await
                .map(|_| ())
                .map_err(Error::from),
            Self::Interaction { command } => command
                .error(ctx, content)
                .await
                .map(|_| ())
                .map_err(Error::from),
        }
    }

    /// Respond with a red embed.
    ///
    /// In case of an interaction, be sure this is the first and only time you call this.
    /// The response will not be ephemeral.
    pub async fn error_callback(&self, ctx: &Context, content: impl Into<String>) -> BotResult<()> {
        match self {
            CommandOrigin::Message { msg } => msg
                .error(ctx, content)
                .await
                .map(|_| ())
                .map_err(Error::from),
            CommandOrigin::Interaction { command } => command
                .error_callback(ctx, content)
                .await
                .map(|_| ())
                .map_err(Error::from),
        }
    }
}

impl<'d> From<&'d mut InteractionCommand> for CommandOrigin<'d> {
    #[inline]
    fn from(command: &'d mut InteractionCommand) -> Self {
        Self::Interaction { command }
    }
}

impl<'d> From<&'d Message> for CommandOrigin<'d> {
    #[inline]
    fn from(msg: &'d Message) -> Self {
        Self::Message { msg }
    }
}
