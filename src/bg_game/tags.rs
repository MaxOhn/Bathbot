#![allow(non_upper_case_globals)]

use std::str::FromStr;

use bitflags::bitflags;

use crate::util::CowUtils;

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
