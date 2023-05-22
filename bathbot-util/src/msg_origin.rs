use std::fmt::{Display, Formatter, Result as FmtResult};

use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker},
    Id,
};

#[derive(Copy, Clone)]
pub struct MessageOrigin {
    guild: Option<Id<GuildMarker>>,
    channel: Id<ChannelMarker>,
}

impl MessageOrigin {
    pub fn new(guild: Option<Id<GuildMarker>>, channel: Id<ChannelMarker>) -> Self {
        Self { guild, channel }
    }
}

impl Display for MessageOrigin {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let Self { guild, channel } = self;

        match guild {
            Some(guild) => write!(f, "https://discord.com/channels/{guild}/{channel}/#"),
            None => write!(f, "https://discord.com/channels/@me/{channel}/#"),
        }
    }
}
