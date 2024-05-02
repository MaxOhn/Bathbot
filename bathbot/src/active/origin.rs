use bathbot_util::{constants::CANNOT_DM_USER, MessageBuilder};
use eyre::Report;
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType,
    Error, Response,
};
use twilight_model::{
    channel::Message,
    id::{marker::ChannelMarker, Id},
};

use crate::{
    core::commands::CommandOrigin,
    util::{interaction::InteractionCommand, ChannelExt},
};

pub enum ActiveMessageOrigin<'d> {
    Channel(Id<ChannelMarker>),
    Command(CommandOrigin<'d>),
}

impl ActiveMessageOrigin<'_> {
    pub(super) async fn create_message(
        &self,
        builder: MessageBuilder<'_>,
    ) -> Result<Response<Message>, ActiveMessageOriginError> {
        match self {
            Self::Channel(channel) => channel.create_message(builder, None).await.map_err(|err| {
                if cannot_dm(&err) {
                    ActiveMessageOriginError::CannotDmUser
                } else {
                    let wrap = "Failed to create message as response";

                    Report::new(err).wrap_err(wrap).into()
                }
            }),
            Self::Command(orig) => orig
                .create_message(builder)
                .await
                .map_err(ActiveMessageOriginError::Report),
        }
    }

    pub(super) async fn callback(
        &self,
        builder: MessageBuilder<'_>,
    ) -> Result<Response<Message>, ActiveMessageOriginError> {
        match self {
            Self::Channel(channel) => channel.create_message(builder, None).await.map_err(|err| {
                if cannot_dm(&err) {
                    ActiveMessageOriginError::CannotDmUser
                } else {
                    let wrap = "Failed to create message as response";

                    Report::new(err).wrap_err(wrap).into()
                }
            }),
            Self::Command(orig) => orig
                .callback_with_response(builder)
                .await
                .map_err(ActiveMessageOriginError::Report),
        }
    }
}

impl<'d> From<CommandOrigin<'d>> for ActiveMessageOrigin<'d> {
    fn from(command: CommandOrigin<'d>) -> Self {
        Self::Command(command)
    }
}

impl From<Id<ChannelMarker>> for ActiveMessageOrigin<'_> {
    fn from(channel: Id<ChannelMarker>) -> Self {
        Self::Channel(channel)
    }
}

impl<'d> From<&'d mut InteractionCommand> for ActiveMessageOrigin<'d> {
    fn from(command: &'d mut InteractionCommand) -> Self {
        Self::Command(command.into())
    }
}

impl<'d> From<&'d Message> for ActiveMessageOrigin<'d> {
    fn from(msg: &'d Message) -> Self {
        Self::Command(msg.into())
    }
}

fn cannot_dm(err: &Error) -> bool {
    matches!(
        err.kind(),
        ErrorType::Response {
            error: ApiError::General(GeneralApiError {
                code: CANNOT_DM_USER,
                ..
            }),
            ..
        }
    )
}

#[derive(Debug, thiserror::Error)]
pub enum ActiveMessageOriginError {
    #[error(transparent)]
    Report(#[from] Report),
    #[error("Cannot send messages to this user")]
    CannotDmUser,
}
