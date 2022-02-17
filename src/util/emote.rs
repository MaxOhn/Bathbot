use std::{borrow::Cow, str::FromStr};

use rosu_v2::prelude::GameMode;
use serde::{
    de::{Deserializer, Error as SerdeError, Unexpected},
    Deserialize,
};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::{channel::ReactionType, id::Id};

use crate::CONFIG;

use super::constants::common_literals::OSU;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Emote {
    Std,
    Tko,
    Ctb,
    Mna,

    Osu,
    Twitch,
    Tracking,

    Minimize,
    Expand,

    JumpStart,
    MultiStepBack,
    SingleStepBack,
    MyPosition,
    SingleStep,
    MultiStep,
    JumpEnd,

    Custom(&'static str),
}

impl Emote {
    pub fn text(self) -> Cow<'static, str> {
        if let Self::Custom(emote) = self {
            format!(":{emote}:").into()
        } else {
            CONFIG.get().unwrap().emotes.get(&self).unwrap().into()
        }
    }

    pub fn request_reaction_type(&self) -> RequestReactionType<'_> {
        let emotes = &CONFIG.get().unwrap().emotes;

        let emote = if let Self::Custom(name) = self {
            return RequestReactionType::Unicode { name };
        } else {
            emotes.get(self)
        };

        let (id, name) = emote
            .unwrap_or_else(|| panic!("No {self:?} emote in config"))
            .split_emote();

        RequestReactionType::Custom {
            id: Id::new(id),
            name: Some(name),
        }
    }

    #[allow(dead_code)]
    pub fn reaction_type(&self) -> ReactionType {
        let emotes = &CONFIG.get().unwrap().emotes;

        let emote = if let Self::Custom(name) = self {
            return ReactionType::Unicode {
                name: name.to_string(),
            };
        } else {
            emotes.get(self)
        };

        let (id, name) = emote
            .unwrap_or_else(|| panic!("No {self:?} emote in config"))
            .split_emote();

        ReactionType::Custom {
            animated: false,
            id: Id::new(id),
            name: Some(name.to_owned()),
        }
    }
}

impl From<GameMode> for Emote {
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::STD => Self::Std,
            GameMode::TKO => Self::Tko,
            GameMode::CTB => Self::Ctb,
            GameMode::MNA => Self::Mna,
        }
    }
}

trait SplitEmote {
    fn split_emote(&self) -> (u64, &str);
}

impl SplitEmote for String {
    fn split_emote(&self) -> (u64, &str) {
        let mut split = self.split(':');
        let name = split.nth(1).unwrap();
        let id = split.next().unwrap();
        let id = u64::from_str(&id[0..id.len() - 1]).unwrap();

        (id, name)
    }
}

impl<'de> Deserialize<'de> for Emote {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        let other = match s {
            OSU => Self::Osu,
            "osu_std" => Self::Std,
            "osu_taiko" => Self::Tko,
            "osu_ctb" => Self::Ctb,
            "osu_mania" => Self::Mna,
            "twitch" => Self::Twitch,
            "tracking" => Self::Tracking,
            "minimize" => Self::Minimize,
            "expand" => Self::Expand,
            "jump_start" => Self::JumpStart,
            "multi_step_back" => Self::MultiStepBack,
            "single_step_back" => Self::SingleStepBack,
            "my_position" => Self::MyPosition,
            "single_step" => Self::SingleStep,
            "multi_step" => Self::MultiStep,
            "jump_end" => Self::JumpEnd,
            other => {
                return Err(SerdeError::invalid_value(
                    Unexpected::Str(other),
                    &"the name of a required emote",
                ))
            }
        };

        Ok(other)
    }
}
