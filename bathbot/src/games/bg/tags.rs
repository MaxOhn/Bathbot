#![allow(non_upper_case_globals)]

use std::str::FromStr;

use bathbot_psql::model::games::{DbMapTagEntry, DbMapTagsParams};
use bitflags::bitflags;
use rosu_v2::prelude::GameMode;

use crate::util::CowUtils;

pub struct MapsetTagsEntries {
    pub mode: GameMode,
    pub tags: Vec<DbMapTagEntry>,
}

bitflags! {
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

impl MapsetTags {
    pub fn as_include(self, params: &mut DbMapTagsParams) {
        macro_rules! set_params {
            ( $( $field:ident: $variant:ident ,)* ) => {
                $(
                    if self.contains(Self::$variant) {
                        params.$field = Some(true);
                    }
                )*
            };
        }

        set_params! {
            farm: Farm,
            alternate: Alternate,
            streams: Streams,
            old: Old,
            meme: Meme,
            hardname: HardName,
            kpop: Kpop,
            english: English,
            bluesky: BlueSky,
            weeb: Weeb,
            tech: Tech,
            easy: Easy,
            hard: Hard,
        }
    }

    pub fn as_exclude(self, params: &mut DbMapTagsParams) {
        macro_rules! set_params {
            ( $( $field:ident: $variant:ident ,)* ) => {
                $(
                    if self.contains(Self::$variant) {
                        params.$field = Some(false);
                    }
                )*
            };
        }

        set_params! {
            farm: Farm,
            alternate: Alternate,
            streams: Streams,
            old: Old,
            meme: Meme,
            hardname: HardName,
            kpop: Kpop,
            english: English,
            bluesky: BlueSky,
            weeb: Weeb,
            tech: Tech,
            easy: Easy,
            hard: Hard,
        }
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
