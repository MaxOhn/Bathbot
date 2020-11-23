use serde::{Deserialize, Serialize};
use twilight_model::{
    id::UserId,
    user::{CurrentUser, PremiumType, UserFlags},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct ColdStorageCurrentUser {
    #[serde(rename = "a", default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(rename = "b")]
    pub discriminator: String,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UserFlags>,
    #[serde(rename = "d")]
    pub id: UserId,
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(rename = "f")]
    pub mfa_enabled: bool,
    #[serde(rename = "g")]
    pub name: String,
    #[serde(rename = "h", default, skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<PremiumType>,
    #[serde(rename = "i", default, skip_serializing_if = "Option::is_none")]
    pub public_flags: Option<UserFlags>,
    #[serde(rename = "j", default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

impl Into<CurrentUser> for ColdStorageCurrentUser {
    fn into(self) -> CurrentUser {
        CurrentUser {
            avatar: self.avatar,
            bot: true,
            discriminator: self.discriminator,
            email: None,
            flags: self.flags,
            id: self.id,
            locale: self.locale,
            mfa_enabled: self.mfa_enabled,
            name: self.name,
            premium_type: self.premium_type,
            public_flags: self.public_flags,
            verified: self.verified,
        }
    }
}

impl From<CurrentUser> for ColdStorageCurrentUser {
    fn from(user: CurrentUser) -> Self {
        Self {
            avatar: user.avatar,
            discriminator: user.discriminator,
            flags: user.flags,
            id: user.id,
            locale: user.locale,
            mfa_enabled: user.mfa_enabled,
            name: user.name,
            premium_type: user.premium_type,
            public_flags: user.public_flags,
            verified: user.verified,
        }
    }
}
