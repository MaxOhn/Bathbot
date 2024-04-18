#![allow(non_upper_case_globals)]

use std::{
    fmt::{Display, Write},
    str::FromStr,
};

use bathbot_util::CowUtils;
use twilight_interactions::command::{CommandOption, CreateOption};

pub struct BgGameScore {
    pub discord_id: i64,
    pub score: i32,
}

pub struct HlGameScore {
    pub discord_id: i64,
    pub highscore: i32,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum HlVersion {
    #[option(name = "Score PP", value = "score_pp")]
    ScorePp = 0,
}

bitflags::bitflags! {
    pub struct MapsetTags: u32 {
        const Farm =      1 << 0;
        const Streams =   1 << 1;
        const Alternate = 1 << 2;
        const Old =       1 << 3;
        const Meme =      1 << 4;
        const HardName =  1 << 5;
        const Easy =      1 << 6;
        const Hard =      1 << 7;
        const Tech =      1 << 8;
        const Weeb =      1 << 9;
        const BlueSky =   1 << 10;
        const English =   1 << 11;
        const Kpop =      1 << 12;
    }
}

impl Default for MapsetTags {
    #[inline]
    fn default() -> Self {
        Self::all()
    }
}

impl FromStr for MapsetTags {
    type Err = String;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let result = match value.cow_to_ascii_lowercase().as_ref() {
            "farm" => Self::Farm,
            "stream" | "streams" => Self::Streams,
            "alt" | "alternate" => Self::Alternate,
            "old" | "oldschool" => Self::Old,
            "meme" => Self::Meme,
            "hardname" => Self::HardName,
            "easy" => Self::Easy,
            "hard" => Self::Hard,
            "tech" | "technical" => Self::Tech,
            "bluesky" => Self::BlueSky,
            "english" => Self::English,
            "weeb" | "anime" => Self::Weeb,
            "kpop" => Self::Kpop,
            other => return Err(other.to_owned()),
        };

        Ok(result)
    }
}

bitflags::bitflags! {
    pub struct Effects: u8 {
        const Blur           = 1 << 0;
        const Contrast       = 1 << 1;
        const FlipHorizontal = 1 << 2;
        const FlipVertical   = 1 << 3;
        const Grayscale      = 1 << 4;
        const Invert         = 1 << 5;
    }
}

pub struct IntoIter<F> {
    flags: F,
    shift: usize,
}

macro_rules! bitflag_impls {
    ($ty:ident, $size:literal) => {
        impl $ty {
            pub fn join(self, separator: impl Display) -> String {
                let mut iter = self.into_iter();

                let first_flag = match iter.next() {
                    Some(first_flag) => first_flag,
                    None => return "None".to_owned(),
                };

                let size = self.bits().count_ones() as usize;
                let mut result = String::with_capacity(size * 6);
                let _ = write!(result, "{first_flag:?}");

                for element in iter {
                    let _ = write!(result, "{separator}{element:?}");
                }

                result
            }
        }

        impl Iterator for IntoIter<$ty> {
            type Item = $ty;

            fn next(&mut self) -> Option<Self::Item> {
                if self.flags.is_empty() {
                    None
                } else {
                    loop {
                        if self.shift == $size {
                            return None;
                        }

                        let bit = 1 << self.shift;
                        self.shift += 1;

                        if self.flags.bits() & bit != 0 {
                            return $ty::from_bits(bit);
                        }
                    }
                }
            }
        }

        impl IntoIterator for $ty {
            type IntoIter = IntoIter<$ty>;
            type Item = $ty;

            fn into_iter(self) -> IntoIter<$ty> {
                IntoIter {
                    flags: self,
                    shift: 0,
                }
            }
        }
    };
}

bitflag_impls!(MapsetTags, 32);
bitflag_impls!(Effects, 8);
