use twilight_model::{
    application::interaction::{ApplicationCommand, MessageComponentInteraction},
    id::{ChannelId, GuildId, UserId},
};

use crate::{BotResult, Error};

pub trait InteractionExt: Send + Sync {
    fn channel_id(&self) -> ChannelId;
    fn guild_id(&self) -> Option<GuildId>;
    fn user_id(&self) -> BotResult<UserId>;
    fn username(&self) -> BotResult<&str>;
}

impl InteractionExt for ApplicationCommand {
    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    fn user_id(&self) -> BotResult<UserId> {
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
    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    fn user_id(&self) -> BotResult<UserId> {
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
