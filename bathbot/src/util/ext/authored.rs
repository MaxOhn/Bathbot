use eyre::Result;
use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
    user::User,
};

pub trait Authored {
    /// Channel id of the event
    fn channel_id(&self) -> Id<ChannelMarker>;

    /// Guild id of the event
    fn guild_id(&self) -> Option<Id<GuildMarker>>;

    /// Author of the event
    fn user(&self) -> Result<&User>;

    /// Author's user id
    #[inline]
    fn user_id(&self) -> Result<Id<UserMarker>> {
        self.user().map(|user| user.id)
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
    fn user(&self) -> Result<&User> {
        Ok(&self.author)
    }

    #[inline]
    fn user_id(&self) -> Result<Id<UserMarker>> {
        Ok(self.author.id)
    }
}
