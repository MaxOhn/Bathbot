use thiserror::Error;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    Id,
};

#[derive(Debug, Error)]
pub enum CacheMiss {
    #[error("missing channel {channel}")]
    Channel { channel: Id<ChannelMarker> },
    #[error("missing current user")]
    CurrentUser,
    #[error("missing guild {guild}")]
    Guild { guild: Id<GuildMarker> },
    #[error("missing member {user} in guild {guild}")]
    Member {
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    },
    #[error("missing role {role}")]
    Role { role: Id<RoleMarker> },
    #[error("missing user {user}")]
    User { user: Id<UserMarker> },
}
