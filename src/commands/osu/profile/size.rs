use crate::embeds::ProfileEmbed;

use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};

use super::ProfileSize;

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

impl From<i16> for ProfileSize {
    fn from(size: i16) -> Self {
        match size {
            0 => Self::Compact,
            1 => Self::Medium,
            2 => Self::Full,
            _ => Self::Compact,
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
    pub fn entry(&mut self, kind: ProfileSize) -> &mut Option<ProfileEmbed> {
        match kind {
            ProfileSize::Compact => &mut self.compact,
            ProfileSize::Medium => &mut self.medium,
            ProfileSize::Full => &mut self.full,
        }
    }
}
