use std::borrow::Cow;

use itoa::Buffer;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    Id,
};

#[derive(Clone, Debug)]
pub(crate) enum SingleEntry {
    CurrentUser,
    Channel {
        guild: Option<Id<GuildMarker>>,
        channel: Id<ChannelMarker>,
    },
    Guild {
        guild: Id<GuildMarker>,
    },
    Member {
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    },
    ResumeData,
    Role {
        guild: Id<GuildMarker>,
        role: Id<RoleMarker>,
    },
    User {
        user: Id<UserMarker>,
    },
}

impl SingleEntry {
    pub(crate) fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut res = Cow::<'_, [u8]>::default();

        fn push(res: &mut Vec<u8>, slice: &str) {
            res.extend_from_slice(slice.as_bytes());
        }

        match self {
            Self::CurrentUser => res = Cow::Borrowed(b"CURRENT_USER"),
            Self::Channel { guild, channel } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "CHANNEL:");

                match guild {
                    Some(guild) => {
                        push(res, buf.format(guild.get()));
                        res.push(b':');
                        push(res, buf.format(channel.get()));
                    }
                    None => push(res, buf.format(channel.get())),
                }
            }
            Self::Guild { guild } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "GUILD:");
                push(res, buf.format(guild.get()));
            }
            Self::Member { guild, user } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "MEMBER:");
                push(res, buf.format(guild.get()));
                res.push(b':');
                push(res, buf.format(user.get()));
            }
            Self::ResumeData => res = Cow::Borrowed(b"RESUME_DATA"),
            Self::Role { guild, role } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "ROLE:");
                push(res, buf.format(guild.get()));
                res.push(b':');
                push(res, buf.format(role.get()));
            }
            Self::User { user } => {
                let mut buf = Buffer::new();
                let res = res.to_mut();

                push(res, "USER:");
                push(res, buf.format(user.get()));
            }
        }

        res
    }
}
