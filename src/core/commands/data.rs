use crate::{Args, BotResult, Error};

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

impl<'m> CommandData<'m> {
    pub fn guild_id(&self) -> Option<GuildId> {
        match self {
            Self::Message { msg, .. } => msg.guild_id,
            Self::Interaction { command } => command.guild_id,
        }
    }

    pub fn channel_id(&self) -> ChannelId {
        match self {
            Self::Message { msg, .. } => msg.channel_id,
            Self::Interaction { command } => command.channel_id,
        }
    }

    pub fn author(&self) -> BotResult<&User> {
        match self {
            Self::Message { msg, .. } => Ok(&msg.author),
            Self::Interaction { command } => command
                .member
                .as_ref()
                .and_then(|member| member.user.as_ref())
                .or_else(|| command.user.as_ref())
                .ok_or(Error::MissingSlashAuthor),
        }
    }

    pub fn compact(self) -> CommandDataCompact {
        self.into()
    }
}

impl<'m> From<ApplicationCommand> for CommandData<'m> {
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

impl<'m> From<CommandData<'m>> for CommandDataCompact {
    fn from(data: CommandData<'m>) -> Self {
        match data {
            CommandData::Message { msg, .. } => msg.into(),
            CommandData::Interaction { command } => command.into(),
        }
    }
}

impl<'m> From<&'m Message> for CommandDataCompact {
    fn from(msg: &'m Message) -> Self {
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
