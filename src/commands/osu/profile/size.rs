use crate::embeds::ProfileEmbed;

use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProfileSize {
    Compact,
    Medium,
    Full,
}

impl ProfileSize {
    pub fn minimize(&self) -> Option<Self> {
        match self {
            ProfileSize::Compact => None,
            ProfileSize::Medium => Some(ProfileSize::Compact),
            ProfileSize::Full => Some(ProfileSize::Medium),
        }
    }

    pub fn expand(&self) -> Option<Self> {
        match self {
            ProfileSize::Compact => Some(ProfileSize::Medium),
            ProfileSize::Medium => Some(ProfileSize::Full),
            ProfileSize::Full => None,
        }
    }
}

impl Default for ProfileSize {
    fn default() -> Self {
        Self::Compact
    }
}

impl Serialize for ProfileSize {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ProfileSize::Compact => s.serialize_u8(0),
            ProfileSize::Medium => s.serialize_u8(1),
            ProfileSize::Full => s.serialize_u8(2),
        }
    }
}

impl<'de> Deserialize<'de> for ProfileSize {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match <u8 as Deserialize>::deserialize(d) {
            Ok(0) => Ok(Self::Compact),
            Ok(1) => Ok(Self::Medium),
            Ok(2) => Ok(Self::Full),
            Ok(_) | Err(_) => Err(Error::custom(
                "expected 0, 1, or 2 when deserializing ProfileSize",
            )),
        }
    }
}

#[derive(Default)]
pub struct ProfileEmbedMap {
    compact: Option<ProfileEmbed>,
    medium: Option<ProfileEmbed>,
    full: Option<ProfileEmbed>,
}

impl ProfileEmbedMap {
    pub fn get(&self, kind: ProfileSize) -> Option<&ProfileEmbed> {
        match kind {
            ProfileSize::Compact => self.compact.as_ref(),
            ProfileSize::Medium => self.medium.as_ref(),
            ProfileSize::Full => self.full.as_ref(),
        }
    }

    pub fn insert(&mut self, kind: ProfileSize, embed: ProfileEmbed) -> &ProfileEmbed {
        match kind {
            ProfileSize::Compact => self.compact.insert(embed),
            ProfileSize::Medium => self.medium.insert(embed),
            ProfileSize::Full => self.full.insert(embed),
        }
    }
}
