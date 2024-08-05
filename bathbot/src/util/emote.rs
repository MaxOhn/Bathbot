use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::ReactionType,
    id::{marker::EmojiMarker, Id},
};

use crate::core::BotConfig;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
#[repr(u8)]
pub enum Emote {
    Std,
    Tko,
    Ctb,
    Mna,

    Osu,
    Twitch,
    Tracking,

    JumpStart,
    SingleStepBack,
    MyPosition,
    SingleStep,
    JumpEnd,

    Miss,
    Bpm,
    CountObjects,
    CountSpinners,
}

impl Emote {
    pub fn reaction_type(self) -> ReactionType {
        let CustomEmote { id, name } = BotConfig::get().emote(self);

        ReactionType::Custom {
            animated: false,
            id: *id,
            name: Some(name.as_ref().to_owned()),
        }
    }

    pub fn url(self) -> String {
        let id = BotConfig::get().emote(self).id;

        format!("https://cdn.discordapp.com/emojis/{id}.png")
    }
}

impl From<GameMode> for Emote {
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::Osu => Self::Std,
            GameMode::Taiko => Self::Tko,
            GameMode::Catch => Self::Ctb,
            GameMode::Mania => Self::Mna,
        }
    }
}

impl FromStr for Emote {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let emote = match s {
            "osu" => Self::Osu,
            "osu_std" => Self::Std,
            "osu_taiko" => Self::Tko,
            "osu_ctb" => Self::Ctb,
            "osu_mania" => Self::Mna,
            "twitch" => Self::Twitch,
            "tracking" => Self::Tracking,
            "jump_start" => Self::JumpStart,
            "single_step_back" => Self::SingleStepBack,
            "my_position" => Self::MyPosition,
            "single_step" => Self::SingleStep,
            "jump_end" => Self::JumpEnd,
            "miss" => Self::Miss,
            "bpm" => Self::Bpm,
            "count_objects" => Self::CountObjects,
            "count_spinners" => Self::CountSpinners,
            _ => return Err(()),
        };

        Ok(emote)
    }
}

impl Display for Emote {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let CustomEmote { id, name } = BotConfig::get().emote(*self);

        write!(f, "<:{name}:{id}>")
    }
}

#[derive(Debug)]
pub struct CustomEmote {
    id: Id<EmojiMarker>,
    name: Box<str>,
}

impl CustomEmote {
    pub fn new(id: u64, name: Box<str>) -> Self {
        Self {
            id: Id::new(id),
            name,
        }
    }
}
