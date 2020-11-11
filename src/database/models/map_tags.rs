use crate::bg_game::MapsetTags;

use rosu::model::GameMode;
use sqlx::{postgres::PgRow, Error, FromRow};
use std::{fmt, ops::Deref};

pub struct MapsetTagWrapper {
    pub mapset_id: u32,
    pub mode: GameMode,
    pub filetype: String,
    pub tags: MapsetTags,
}

impl Deref for MapsetTagWrapper {
    type Target = MapsetTags;
    fn deref(&self) -> &Self::Target {
        &self.tags
    }
}

impl MapsetTagWrapper {
    pub fn untagged(&self) -> bool {
        self.tags.is_empty()
    }
    #[allow(dead_code)]
    pub fn any(&self) -> bool {
        !self.tags.is_empty()
    }
    #[allow(dead_code)]
    pub fn has_tags(&self, tags: MapsetTags) -> bool {
        self.contains(tags)
    }
}

impl<'c> FromRow<'c, PgRow> for MapsetTagWrapper {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        let row = TagRow::from_row(row)?;
        let bits = row.farm as u32
            + ((row.streams as u32) << 1)
            + ((row.alternate as u32) << 2)
            + ((row.old as u32) << 3)
            + ((row.meme as u32) << 4)
            + ((row.hardname as u32) << 5)
            + ((row.easy as u32) << 6)
            + ((row.hard as u32) << 7)
            + ((row.tech as u32) << 8)
            + ((row.weeb as u32) << 9)
            + ((row.bluesky as u32) << 10)
            + ((row.english as u32) << 11)
            + ((row.kpop as u32) << 12);
        Ok(Self {
            mapset_id: row.beatmapset_id,
            mode: (row.mode as u8).into(),
            tags: MapsetTags::from_bits(bits).unwrap(),
            filetype: row.filetype,
        })
    }
}

impl fmt::Display for MapsetTagWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tags.join(", "))
    }
}

#[derive(FromRow)]
struct TagRow {
    beatmapset_id: u32,
    mode: i8,
    filetype: String,
    farm: bool,
    alternate: bool,
    streams: bool,
    old: bool,
    meme: bool,
    hardname: bool,
    kpop: bool,
    english: bool,
    bluesky: bool,
    weeb: bool,
    tech: bool,
    easy: bool,
    hard: bool,
}
