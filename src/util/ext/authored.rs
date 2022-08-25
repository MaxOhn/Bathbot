use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
    user::User,
};

use crate::BotResult;

pub trait Authored {
    /// Channel id of the event
    fn channel_id(&self) -> Id<ChannelMarker>;

    /// Guild id of the event
    fn guild_id(&self) -> Option<Id<GuildMarker>>;

    /// Author of the event
    fn user(&self) -> BotResult<&User>;

    /// Author's user id
    #[inline]
    fn user_id(&self) -> BotResult<Id<UserMarker>> {
        self.user().map(|user| user.id)
    }

    /// Author's username
    #[inline]
    fn username(&self) -> BotResult<&str> {
        self.user().map(|user| user.name.as_str())
    }
}

impl Authored for Message {
    #[inline]
    fn channel_id(&self) -> Id<ChannelMarker> {
        self.channel_id
    }

    #[inline]
    fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.guild_id
    }

    #[inline]
    fn user(&self) -> BotResult<&User> {
        Ok(&self.author)
    }

    #[inline]
    fn user_id(&self) -> BotResult<Id<UserMarker>> {
        Ok(self.author.id)
    }

    #[inline]
    fn username(&self) -> BotResult<&str> {
        Ok(self.author.name.as_str())
    }
}
