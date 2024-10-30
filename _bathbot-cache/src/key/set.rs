use std::borrow::Cow;

use itoa::Buffer;
use twilight_model::id::{marker::GuildMarker, Id};

#[derive(Clone, Debug)]
pub(crate) enum SetEntry {
    Channels,
    Guilds,
    GuildChannels { guild: Id<GuildMarker> },
    GuildMembers { guild: Id<GuildMarker> },
    GuildRoles { guild: Id<GuildMarker> },
    Roles,
    UnavailableGuilds,
    Users,
}

impl SetEntry {
    pub(crate) fn to_bytes(&self) -> Cow<'static, [u8]> {
        let mut res = Cow::<'_, [u8]>::default();

        fn push(res: &mut Vec<u8>, slice: &str) {
            res.extend_from_slice(slice.as_bytes());
        }

        match self {
            SetEntry::Channels => res = Cow::Borrowed(b"CHANNEL_IDS"),
            SetEntry::Guilds => res = Cow::Borrowed(b"GUILD_IDS"),
            SetEntry::GuildChannels { guild } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "GUILD_CHANNELS:");
                push(res, buf.format(guild.get()));
            }
            SetEntry::GuildMembers { guild } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "GUILD_MEMBERS:");
                push(res, buf.format(guild.get()));
            }
            SetEntry::GuildRoles { guild } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "GUILD_ROLES:");
                push(res, buf.format(guild.get()));
            }
            SetEntry::Roles => res = Cow::Borrowed(b"ROLE_IDS"),
            SetEntry::UnavailableGuilds => res = Cow::Borrowed(b"UNAVAILABLE_GUILD_IDS"),
            SetEntry::Users => res = Cow::Borrowed(b"USER_IDS"),
        }

        res
    }
}
