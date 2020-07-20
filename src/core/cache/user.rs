use super::is_default;

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
    #[serde(rename = "d")]
    // TODO: Store as u16
    pub discriminator: String,
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
        CachedUser {
            id: user.id,
            username: user.name.clone(),
            discriminator: user.discriminator.clone(),
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
            && self.discriminator == user.discriminator
            && self.avatar == user.avatar
            && self.bot_user == user.bot
            && self.system_user == user.system.unwrap_or(false)
            && self.public_flags == user.public_flags
    }
}
