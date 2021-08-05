use crate::bg_game::MapsetTags;

use rosu_v2::model::GameMode;
use sqlx::{postgres::PgRow, Error, FromRow};
use std::{fmt, ops::Deref};

pub struct MapsetTagWrapper {
    pub mapset_id: u32,
    pub mode: GameMode,
    pub filename: String,
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
}

impl From<TagRow> for MapsetTagWrapper {
    fn from(row: TagRow) -> Self {
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

        Self {
            mapset_id: row.mapset_id as u32,
            mode: (row.mode as u8).into(),
            tags: MapsetTags::from_bits(bits).unwrap(),
            filename: row.filename,
        }
    }
}

impl<'c> FromRow<'c, PgRow> for MapsetTagWrapper {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        TagRow::from_row(row).map(From::from)
    }
}

impl fmt::Display for MapsetTagWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tags.join(", "))
    }
}

#[derive(FromRow)]
pub struct TagRow {
    pub mapset_id: i32,
    pub mode: i16,
    pub filename: String,
    pub farm: bool,
    pub alternate: bool,
    pub streams: bool,
    pub old: bool,
    pub meme: bool,
    pub hardname: bool,
    pub kpop: bool,
    pub english: bool,
    pub bluesky: bool,
    pub weeb: bool,
    pub tech: bool,
    pub easy: bool,
    pub hard: bool,
}
