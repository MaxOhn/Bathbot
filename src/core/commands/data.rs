use crate::{util, Args, BotResult, Error};

use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    id::{ChannelId, GuildId, InteractionId, MessageId},
    user::User,
};

pub enum CommandData<'m> {
    Message {
        msg: &'m Message,
        args: Args<'m>,
        num: Option<usize>,
    },
    Interaction {
        command: ApplicationCommand,
    },
}

impl CommandData<'_> {
    pub fn guild_id(&self) -> Option<GuildId> {
        util::Authored::guild_id(self)
    }

    pub fn channel_id(&self) -> ChannelId {
        util::Authored::channel_id(self)
    }

    pub fn author(&self) -> BotResult<&User> {
        util::Authored::author(self).ok_or(Error::MissingSlashAuthor)
    }

    pub fn compact(self) -> CommandDataCompact {
        self.into()
    }
}

impl From<ApplicationCommand> for CommandData<'_> {
    fn from(command: ApplicationCommand) -> Self {
        Self::Interaction { command }
    }
}

pub enum CommandDataCompact {
    Message {
        msg_id: MessageId,
        channel_id: ChannelId,
    },
    Interaction {
        interaction_id: InteractionId,
        token: String,
    },
}

impl From<CommandData<'_>> for CommandDataCompact {
    fn from(data: CommandData<'_>) -> Self {
        match data {
            CommandData::Message { msg, .. } => msg.into(),
            CommandData::Interaction { command } => command.into(),
        }
    }
}

impl From<&Message> for CommandDataCompact {
    fn from(msg: &Message) -> Self {
        Self::Message {
            msg_id: msg.id,
            channel_id: msg.channel_id,
        }
    }
}

impl From<ApplicationCommand> for CommandDataCompact {
    fn from(command: ApplicationCommand) -> Self {
        Self::Interaction {
            interaction_id: command.id,
            token: command.token,
        }
    }
}
