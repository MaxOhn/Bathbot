use crate::CommandData;

use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
    user::User,
};

pub trait Authored {
    fn author(&self) -> Option<&User>;
    fn guild_id(&self) -> Option<Id<GuildMarker>>;
    fn channel_id(&self) -> Id<ChannelMarker>;
}

impl Authored for Message {
    fn author(&self) -> Option<&User> {
        Some(&self.author)
    }

    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.guild_id
    }

    fn channel_id(&self) -> Id<ChannelMarker> {
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

    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.guild_id
    }

    fn channel_id(&self) -> Id<ChannelMarker> {
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

    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        match self {
            CommandData::Message { msg, .. } => msg.guild_id(),
            CommandData::Interaction { command } => command.guild_id(),
        }
    }

    fn channel_id(&self) -> Id<ChannelMarker> {
        match self {
            CommandData::Message { msg, .. } => msg.channel_id(),
            CommandData::Interaction { command } => command.channel_id(),
        }
    }
}
