use super::{get_true, is_default, is_true};

use serde::{Deserialize, Serialize};
use std::fmt;
use twilight_model::{
    guild::Emoji,
    id::{EmojiId, RoleId},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedEmoji {
    #[serde(rename = "a")]
    pub id: EmojiId,
    #[serde(rename = "b")]
    pub name: String,
    #[serde(rename = "c", default, skip_serializing_if = "is_default")]
    pub roles: Vec<RoleId>,
    #[serde(rename = "i", default, skip_serializing_if = "is_default")]
    pub requires_colons: bool,
    #[serde(rename = "j", default, skip_serializing_if = "is_default")]
    pub managed: bool,
    #[serde(rename = "k", default, skip_serializing_if = "is_default")]
    pub animated: bool,
    #[serde(rename = "l", default = "get_true", skip_serializing_if = "is_true")]
    pub available: bool,
}

impl From<Emoji> for CachedEmoji {
    fn from(emoji: Emoji) -> Self {
        CachedEmoji {
            id: emoji.id,
            name: emoji.name,
            roles: emoji.roles,
            requires_colons: emoji.require_colons,
            managed: emoji.managed,
            animated: emoji.animated,
            available: emoji.available,
        }
    }
}

impl fmt::Display for CachedEmoji {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.name)
    }
}
