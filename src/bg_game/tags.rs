#![allow(non_upper_case_globals)]

use crate::util::CowUtils;

use bitflags::bitflags;
use std::{fmt::Write, str::FromStr};

bitflags! {
    pub struct MapsetTags: u32 {
        const Farm = 1;
        const Streams = 2;
        const Alternate = 4;
        const Old = 8;
        const Meme = 16;
        const HardName = 32;
        const Easy = 64;
        const Hard = 128;
        const Tech = 256;
        const Weeb = 512;
        const BlueSky = 1024;
        const English = 2048;
        const Kpop = 4096;
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

impl MapsetTags {
    #[inline]
    pub fn size(self) -> usize {
        self.bits().count_ones() as usize
    }

    pub fn join(self, separator: impl std::fmt::Display) -> String {
        let mut iter = self.into_iter();

        let first_tag = match iter.next() {
            Some(first_tag) => first_tag,
            None => return "None".to_owned(),
        };

        let mut result = String::with_capacity(self.size() * 6);
        let _ = write!(result, "{:?}", first_tag);

        for element in iter {
            let _ = write!(result, "{}{:?}", separator, element);
        }

        result
    }
}

pub struct IntoIter {
    tags: MapsetTags,
    shift: usize,
}

impl Iterator for IntoIter {
    type Item = MapsetTags;

    fn next(&mut self) -> Option<Self::Item> {
        if self.tags.is_empty() {
            None
        } else {
            loop {
                if self.shift == 32 {
                    return None;
                }

                let bit = 1 << self.shift;
                self.shift += 1;
                if self.tags.bits & bit != 0 {
                    return MapsetTags::from_bits(bit);
                }
            }
        }
    }
}

impl IntoIterator for MapsetTags {
    type Item = MapsetTags;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        IntoIter {
            tags: self,
            shift: 0,
        }
    }
}
