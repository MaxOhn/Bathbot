use twilight_model::{
    application::interaction::{ApplicationCommand, MessageComponentInteraction},
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};

use crate::{BotResult, Error};

pub trait InteractionExt: Send + Sync {
    fn channel_id(&self) -> Id<ChannelMarker>;
    fn guild_id(&self) -> Option<Id<GuildMarker>>;
    fn user_id(&self) -> BotResult<Id<UserMarker>>;
    fn username(&self) -> BotResult<&str>;
}

impl InteractionExt for ApplicationCommand {
    fn channel_id(&self) -> Id<ChannelMarker> {
        self.channel_id
    }

    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.guild_id
    }

    fn user_id(&self) -> BotResult<Id<UserMarker>> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.id)
            .ok_or(Error::MissingInteractionAuthor)
    }

    fn username(&self) -> BotResult<&str> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.name.as_str())
            .ok_or(Error::MissingInteractionAuthor)
    }
}

impl InteractionExt for MessageComponentInteraction {
    fn channel_id(&self) -> Id<ChannelMarker> {
        self.channel_id
    }

    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.guild_id
    }

    fn user_id(&self) -> BotResult<Id<UserMarker>> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.id)
            .ok_or(Error::MissingInteractionAuthor)
    }

    fn username(&self) -> BotResult<&str> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or_else(|| self.user.as_ref())
            .map(|user| user.name.as_str())
            .ok_or(Error::MissingInteractionAuthor)
    }
}
