use super::{is_default, serde::deserialize_u16};

use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU64;
use twilight::model::{
    id::UserId,
    user::{User, UserFlags},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedUser {
    #[serde(rename = "i")]
    pub id: UserId,
    #[serde(rename = "u")]
    pub username: String,
    #[serde(rename = "d", deserialize_with = "deserialize_u16")]
    pub discriminator: u16,
    #[serde(rename = "a", default, skip_serializing_if = "is_default")]
    pub avatar: Option<String>,
    #[serde(rename = "b", default, skip_serializing_if = "is_default")]
    pub bot_user: bool,
    #[serde(rename = "s", default, skip_serializing_if = "is_default")]
    pub system_user: bool,
    #[serde(rename = "f", default, skip_serializing_if = "is_default")]
    pub public_flags: Option<UserFlags>,
    #[serde(skip_serializing, default)]
    pub mutual_servers: AtomicU64,
}

impl CachedUser {
    pub(crate) fn from_user(user: &User) -> Self {
        let discriminator = match user.discriminator.parse::<u16>() {
            Ok(num) => num,
            Err(_) => {
                error!(
                    "Could not parse descriminator `{}` as u16, replace with 0",
                    user.discriminator
                );
                0
            }
        };
        CachedUser {
            id: user.id,
            username: user.name.clone(),
            discriminator,
            avatar: user.avatar.clone(),
            bot_user: user.bot,
            system_user: user.system.unwrap_or(false),
            public_flags: user.public_flags,
            mutual_servers: AtomicU64::new(0),
        }
    }

    pub fn is_same_as(&self, user: &User) -> bool {
        self.id == user.id
            && self.username == user.name
            && self.avatar == user.avatar
            && self.bot_user == user.bot
            && self.system_user == user.system.unwrap_or(false)
            && self.public_flags == user.public_flags
            && self.discriminator.to_string() == user.discriminator
    }
}
