use twilight_http::{Error as HttpError, Response};
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::Context,
    util::{
        builder::MessageBuilder, ApplicationCommandExt, Authored, ChannelExt, ComponentExt,
        MessageExt,
    },
    BotResult,
};

type HttpResult<T> = Result<T, HttpError>;

pub enum CommandOrigin<'m> {
    Message { msg: &'m Message },
    Interaction { command: Box<ApplicationCommand> },
}

impl CommandOrigin<'_> {
    pub fn user_id(&self) -> BotResult<Id<UserMarker>> {
        match self {
            CommandOrigin::Message { msg } => Ok(msg.author.id),
            CommandOrigin::Interaction { command } => command.user_id(),
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
    pub async fn error(&self, ctx: &Context, content: impl Into<String>) -> HttpResult<()> {
        match self {
            Self::Message { msg } => msg.error(ctx, content).await.map(|_| ()),
            Self::Interaction { command } => command.error(ctx, content).await.map(|_| ()),
        }
    }

    /// Respond with a red embed.
    ///
    /// In case of an interaction, be sure this is the first and only time you call this.
    /// The response will not be ephemeral.
    pub async fn error_callback(
        &self,
        ctx: &Context,
        content: impl Into<String>,
    ) -> HttpResult<()> {
        match self {
            CommandOrigin::Message { msg } => msg.error(ctx, content).await.map(|_| ()),
            CommandOrigin::Interaction { command } => {
                command.error_callback(ctx, content).await.map(|_| ())
            }
        }
    }
}

impl From<Box<ApplicationCommand>> for CommandOrigin<'_> {
    #[inline]
    fn from(command: Box<ApplicationCommand>) -> Self {
        Self::Interaction { command }
    }
}

impl<'m> From<&'m Message> for CommandOrigin<'m> {
    #[inline]
    fn from(msg: &'m Message) -> Self {
        Self::Message { msg }
    }
}
