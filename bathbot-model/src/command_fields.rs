use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    str::FromStr,
};

use rosu_v2::prelude::{GameMode, Grade};
use serde::{
    de::{Deserializer, Error as DeError, Unexpected},
    ser::Serializer,
    Deserialize, Serialize,
};
use time::UtcOffset;
use twilight_interactions::command::{CommandOption, CreateOption};

use crate::deser::bool_as_u8;

macro_rules! define_enum {
    (
        #[$enum_meta:meta]
        pub enum $enum_name:ident {
            $(
                #[$variant_meta:meta]
                $variant:ident = $discriminant:literal,
            )*
        }
    ) => {
        #[$enum_meta]
        pub enum $enum_name {
            $(
                #[$variant_meta]
                $variant = $discriminant,
            )*
        }

        impl<'de> Deserialize<'de> for $enum_name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                match u8::deserialize(d)? {
                    $( $discriminant => Ok(Self::$variant), )*
                    other => Err(DeError::invalid_value(
                        Unexpected::Unsigned(u64::from(other)),
                        &stringify!($enum_name),
                    )),
                }
            }
        }

        impl Serialize for $enum_name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_u8(*self as u8)
            }
        }
    }
}

define_enum! {
    #[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
    pub enum ShowHideOption {
        #[option(name = "Show", value = "show")]
        Show = 0,
        #[option(name = "Hide", value = "hide")]
        Hide = 1,
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum EnableDisable {
    #[option(name = "Enable", value = "enable")]
    Enable,
    #[option(name = "Disable", value = "disable")]
    Disable,
}

#[derive(CommandOption, CreateOption)]
pub enum ThreadChannel {
    #[option(name = "Stay in channel", value = "channel")]
    Channel,
    #[option(name = "Start new thread", value = "thread")]
    Thread,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum GameModeOption {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
    #[option(name = "mania", value = "mania")]
    Mania,
}

impl From<GameModeOption> for GameMode {
    #[inline]
    fn from(mode: GameModeOption) -> Self {
        match mode {
            GameModeOption::Osu => Self::Osu,
            GameModeOption::Taiko => Self::Taiko,
            GameModeOption::Catch => Self::Catch,
            GameModeOption::Mania => Self::Mania,
        }
    }
}

impl From<GameMode> for GameModeOption {
    #[inline]
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::Osu => Self::Osu,
            GameMode::Taiko => Self::Taiko,
            GameMode::Catch => Self::Catch,
            GameMode::Mania => Self::Mania,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum GradeOption {
    #[option(name = "SS", value = "ss")]
    SS,
    #[option(name = "S", value = "s")]
    S,
    #[option(name = "A", value = "a")]
    A,
    #[option(name = "B", value = "b")]
    B,
    #[option(name = "C", value = "c")]
    C,
    #[option(name = "D", value = "d")]
    D,
    #[option(name = "F", value = "f")]
    F,
}

impl From<GradeOption> for Grade {
    #[inline]
    fn from(grade: GradeOption) -> Self {
        match grade {
            GradeOption::SS => Self::X,
            GradeOption::S => Self::S,
            GradeOption::A => Self::A,
            GradeOption::B => Self::B,
            GradeOption::C => Self::C,
            GradeOption::D => Self::D,
            GradeOption::F => Self::F,
        }
    }
}

impl FromStr for GradeOption {
    type Err = &'static str;

    // ! Make sure the given strings are lower case
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let grade = match s {
            "x" | "ss" => Self::SS,
            "s" => Self::S,
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "f" => Self::F,
            _ => {
                return Err("Failed to parse `grade`.\n\
                Valid grades are: `SS`, `S`, `A`, `B`, `C`, `D`, or `F`")
            }
        };

        Ok(grade)
    }
}

macro_rules! timezone_option {
    ( $( $variant:ident, $name:literal, $value:literal, $value_str:literal; )* ) => {
        #[derive(CommandOption, CreateOption)]
        pub enum TimezoneOption {
            $(
                #[option(name = $name, value = $value_str)]
                $variant = $value,
            )*
        }

        impl From<TimezoneOption> for UtcOffset {
            #[inline]
            fn from(tz: TimezoneOption) -> Self {
                let seconds = match tz {
                    $(
                        #[allow(clippy::neg_multiply, clippy::erasing_op, clippy::identity_op)]
                        TimezoneOption:: $variant => $value * 3600,
                    )*
                };

                Self::from_whole_seconds(seconds).unwrap()
            }
        }
    }
}

timezone_option! {
    M12, "UTC-12", -12, "-12";
    M11, "UTC-11", -11, "-11";
    M10, "UTC-10", -10, "-10";
    M9, "UTC-9", -9, "-9";
    M8, "UTC-8", -8, "-8";
    M7, "UTC-7", -7, "-7";
    M6, "UTC-6", -6, "-6";
    M5, "UTC-5", -5, "-5";
    M4, "UTC-4", -4, "-4";
    M3, "UTC-3", -3, "-3";
    M2, "UTC-2", -2, "-2";
    M1, "UTC-1", -1, "-1";
    P0, "UTC+0", 0, "0";
    P1, "UTC+1", 1, "1";
    P2, "UTC+2", 2, "2";
    P3, "UTC+3", 3, "3";
    P4, "UTC+4", 4, "4";
    P5, "UTC+5", 5, "5";
    P6, "UTC+6", 6, "6";
    P7, "UTC+7", 7, "7";
    P8, "UTC+8", 8, "8";
    P9, "UTC+9", 9, "9";
    P10, "UTC+10", 10, "10";
    P11, "UTC+11", 11, "11";
    P12, "UTC+12", 12, "12";
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ScoreEmbedSettings {
    pub image: ScoreEmbedImage,
    pub pp: ScoreEmbedPp,
    pub map_info: ScoreEmbedMapInfo,
    pub footer: ScoreEmbedFooter,
    pub buttons: ScoreEmbedButtons,
    pub hitresults: ScoreEmbedHitResults,
}

impl Default for ScoreEmbedSettings {
    fn default() -> Self {
        Self {
            image: ScoreEmbedImage::Thumbnail,
            pp: ScoreEmbedPp::Max,
            map_info: ScoreEmbedMapInfo {
                len: true,
                ar: true,
                cs: true,
                od: true,
                hp: true,
                bpm: true,
                n_obj: false,
                n_spin: false,
            },
            footer: ScoreEmbedFooter::WithMapRankedDate,
            buttons: ScoreEmbedButtons {
                pagination: true,
                render: true,
                miss_analyzer: true,
            },
            hitresults: ScoreEmbedHitResults::OnlyMisses,
        }
    }
}

define_enum! {
    #[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
    pub enum ScoreEmbedHitResults {
        #[option(name = "Full", value = "full")]
        Full = 0,
        #[option(name = "Only misses", value = "miss")]
        OnlyMisses = 1,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ScoreEmbedButtons {
    #[serde(with = "bool_as_u8")]
    pub pagination: bool,
    #[serde(with = "bool_as_u8")]
    pub render: bool,
    #[serde(with = "bool_as_u8")]
    pub miss_analyzer: bool,
}

define_enum! {
    #[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
    pub enum ScoreEmbedFooter {
        #[option(name = "With score date", value = "score_date")]
        WithScoreDate = 0,
        #[option(name = "With score date", value = "ranked_date")]
        WithMapRankedDate = 1,
        #[option(name = "Hide", value = "hide")]
        Hide = 2,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ScoreEmbedMapInfo {
    #[serde(with = "bool_as_u8")]
    pub len: bool,
    #[serde(with = "bool_as_u8")]
    pub ar: bool,
    #[serde(with = "bool_as_u8")]
    pub cs: bool,
    #[serde(with = "bool_as_u8")]
    pub od: bool,
    #[serde(with = "bool_as_u8")]
    pub hp: bool,
    #[serde(with = "bool_as_u8")]
    pub bpm: bool,
    #[serde(with = "bool_as_u8")]
    pub n_obj: bool,
    #[serde(with = "bool_as_u8")]
    pub n_spin: bool,
}

impl ScoreEmbedMapInfo {
    pub fn show(self) -> bool {
        let Self {
            len,
            ar,
            cs,
            od,
            hp,
            bpm,
            n_obj,
            n_spin,
        } = self;

        len || ar || cs || od || hp || bpm || n_obj || n_spin
    }
}

define_enum! {
    #[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
    pub enum ScoreEmbedImage {
        #[option(name = "Image", value = "image")]
        Image = 0,
        #[option(name = "Thumbnail", value = "thumbnail")]
        Thumbnail = 1,
        #[option(name = "None", value = "none")]
        None = 2,
    }
}

impl ScoreEmbedImage {
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "thumbnail" => Some(Self::Thumbnail),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

define_enum! {
    #[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
    pub enum ScoreEmbedPp {
        #[option(name = "Max PP", value = "max_pp")]
        Max = 0,
        #[option(name = "If-FC PP", value = "if_fc")]
        IfFc = 1,
    }
}

impl ScoreEmbedPp {
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "max_pp" => Some(Self::Max),
            "if_fc" => Some(Self::IfFc),
            _ => None,
        }
    }
}

impl Debug for ScoreEmbedPp {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Max => f.write_str("Max PP"),
            Self::IfFc => f.write_str("If-FC PP"),
        }
    }
}
