use crate::CommandData;

use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    id::{ChannelId, GuildId},
    user::User,
};

pub trait Authored {
    fn author(&self) -> Option<&User>;
    fn guild_id(&self) -> Option<GuildId>;
    fn channel_id(&self) -> ChannelId;
}

impl Authored for Message {
    fn author(&self) -> Option<&User> {
        Some(&self.author)
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
}

impl Authored for ApplicationCommand {
    fn author(&self) -> Option<&User> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
}

impl Authored for CommandData<'_> {
    fn author(&self) -> Option<&User> {
        match self {
            CommandData::Message { msg, .. } => msg.author(),
            CommandData::Interaction { command } => command.author(),
        }
    }

    fn guild_id(&self) -> Option<GuildId> {
        match self {
            CommandData::Message { msg, .. } => msg.guild_id(),
            CommandData::Interaction { command } => command.guild_id(),
        }
    }

    fn channel_id(&self) -> ChannelId {
        match self {
            CommandData::Message { msg, .. } => msg.channel_id(),
            CommandData::Interaction { command } => command.channel_id(),
        }
    }
}
