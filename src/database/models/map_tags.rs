use super::{super::schema::map_tags, beatmap::DBMapSet};
use crate::commands::utility::MapsetTags;

use rosu::models::GameMode;
use std::{fmt, ops::Deref};

#[derive(Default, Debug, Clone, Identifiable, Queryable, Associations, Insertable, AsChangeset)]
#[table_name = "map_tags"]
#[belongs_to(DBMapSet, foreign_key = "beatmapset_id")]
#[primary_key(beatmapset_id)]
pub struct MapsetTagDB {
    pub beatmapset_id: u32,
    pub filetype: Option<String>,
    pub mode: Option<u8>,
    pub farm: Option<bool>,
    pub streams: Option<bool>,
    pub alternate: Option<bool>,
    pub old: Option<bool>,
    pub meme: Option<bool>,
    pub hardname: Option<bool>,
    pub easy: Option<bool>,
    pub hard: Option<bool>,
    pub tech: Option<bool>,
    pub weeb: Option<bool>,
    pub bluesky: Option<bool>,
    pub english: Option<bool>,
    pub kpop: Option<bool>,
}

impl MapsetTagDB {
    pub fn new(beatmapset_id: u32) -> Self {
        Self {
            beatmapset_id,
            filetype: None,
            mode: None,
            farm: None,
            streams: None,
            alternate: None,
            old: None,
            meme: None,
            hardname: None,
            easy: None,
            hard: None,
            tech: None,
            weeb: None,
            bluesky: None,
            english: None,
            kpop: None,
        }
    }
    pub fn with_value(beatmapset_id: u32, tags: MapsetTags, value: bool) -> Self {
        let mut result = Self::new(beatmapset_id);
        if tags.contains(MapsetTags::Farm) {
            result.farm = Some(value);
        }
        if tags.contains(MapsetTags::Streams) {
            result.streams = Some(value);
        }
        if tags.contains(MapsetTags::Alternate) {
            result.alternate = Some(value);
        }
        if tags.contains(MapsetTags::Old) {
            result.old = Some(value);
        }
        if tags.contains(MapsetTags::Meme) {
            result.meme = Some(value);
        }
        if tags.contains(MapsetTags::HardName) {
            result.hardname = Some(value);
        }
        if tags.contains(MapsetTags::Easy) {
            result.easy = Some(value);
        }
        if tags.contains(MapsetTags::Hard) {
            result.hard = Some(value);
        }
        if tags.contains(MapsetTags::Tech) {
            result.tech = Some(value);
        }
        if tags.contains(MapsetTags::Weeb) {
            result.weeb = Some(value);
        }
        if tags.contains(MapsetTags::BlueSky) {
            result.bluesky = Some(value);
        }
        if tags.contains(MapsetTags::English) {
            result.english = Some(value);
        }
        if tags.contains(MapsetTags::Kpop) {
            result.kpop = Some(value);
        }
        result
    }
}

pub struct MapsetTagWrapper {
    pub mapset_id: u32,
    pub mode: GameMode,
    pub filetype: String,
    tags: MapsetTags,
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
    pub fn any(&self) -> bool {
        !self.tags.is_empty()
    }
    pub fn has_tags(&self, tags: MapsetTags) -> bool {
        self.contains(tags)
    }
}

impl From<MapsetTagDB> for MapsetTagWrapper {
    fn from(tags: MapsetTagDB) -> Self {
        let bits = (Some(true) == tags.farm) as u32
            + (((Some(true) == tags.streams) as u32) << 1)
            + (((Some(true) == tags.alternate) as u32) << 2)
            + (((Some(true) == tags.old) as u32) << 3)
            + (((Some(true) == tags.meme) as u32) << 4)
            + (((Some(true) == tags.hardname) as u32) << 5)
            + (((Some(true) == tags.easy) as u32) << 6)
            + (((Some(true) == tags.hard) as u32) << 7)
            + (((Some(true) == tags.tech) as u32) << 8)
            + (((Some(true) == tags.weeb) as u32) << 9)
            + (((Some(true) == tags.bluesky) as u32) << 10)
            + (((Some(true) == tags.english) as u32) << 11)
            + (((Some(true) == tags.kpop) as u32) << 12);
        Self {
            mapset_id: tags.beatmapset_id,
            mode: GameMode::from(tags.mode.unwrap()),
            filetype: tags.filetype.unwrap(),
            tags: MapsetTags::from_bits(bits).unwrap(),
        }
    }
}

impl fmt::Display for MapsetTagWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tags.join(", "))
    }
}
