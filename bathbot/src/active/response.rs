use bathbot_util::MessageBuilder;
use twilight_http::response::ResponseFuture;
use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use super::origin::ActiveMessageOrigin;
use crate::{
    core::commands::CommandOrigin,
    util::{InteractionToken, MessageExt},
};

pub struct ActiveResponse {
    pub msg: Id<MessageMarker>,
    pub inner: ActiveResponseInner,
}

pub enum ActiveResponseInner {
    Message { channel: Id<ChannelMarker> },
    Interaction { token: Box<str> },
}

impl ActiveResponse {
    pub fn new(orig: &ActiveMessageOrigin, response: &Message) -> Self {
        let inner = match orig {
            ActiveMessageOrigin::Channel(_)
            | ActiveMessageOrigin::Command(CommandOrigin::Message { .. }) => {
                ActiveResponseInner::Message {
                    channel: response.channel_id,
                }
            }
            ActiveMessageOrigin::Command(CommandOrigin::Interaction { command }) => {
                ActiveResponseInner::Interaction {
                    token: command.token.as_str().into(),
                }
            }
        };

        Self {
            msg: response.id,
            inner,
        }
    }

    pub fn update(self, builder: MessageBuilder<'_>) -> Option<ResponseFuture<Message>> {
        match self.inner {
            ActiveResponseInner::Message { channel } => (self.msg, channel).update(builder, None),
            ActiveResponseInner::Interaction { token } => {
                Some(InteractionToken(&token).update(builder, None))
            }
        }
    }
}
