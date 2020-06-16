use super::{super::schema::map_tags, beatmap::DBMapSet};
use crate::commands::utility::MapsetTag;

use rosu::models::GameMode;
use std::fmt;

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
        }
    }
    pub fn with_value(beatmapset_id: u32, tag: MapsetTag, value: bool) -> Self {
        let mut result = Self::new(beatmapset_id);
        match tag {
            MapsetTag::Farm => result.farm = Some(value),
            MapsetTag::Streams => result.streams = Some(value),
            MapsetTag::Alternate => result.alternate = Some(value),
            MapsetTag::Old => result.old = Some(value),
            MapsetTag::Meme => result.meme = Some(value),
            MapsetTag::HardName => result.hardname = Some(value),
            MapsetTag::Easy => result.easy = Some(value),
            MapsetTag::Hard => result.hard = Some(value),
            MapsetTag::Tech => result.tech = Some(value),
            MapsetTag::Weeb => result.weeb = Some(value),
            MapsetTag::BlueSky => result.bluesky = Some(value),
            MapsetTag::English => result.english = Some(value),
        }
        result
    }
}

pub struct MapsetTags {
    pub mapset_id: u32,
    pub mode: GameMode,
    pub filetype: String,
    tags: u32,
}

impl MapsetTags {
    pub fn untagged(&self) -> bool {
        self.tags == 0
    }
    pub fn any(&self) -> bool {
        self.tags != 0
    }
    pub fn farm(&self) -> bool {
        self.tags & 1 != 0
    }
    pub fn streams(&self) -> bool {
        self.tags & 2 != 0
    }
    pub fn alternate(&self) -> bool {
        self.tags & 4 != 0
    }
    pub fn old(&self) -> bool {
        self.tags & 8 != 0
    }
    pub fn meme(&self) -> bool {
        self.tags & 16 != 0
    }
    pub fn hardname(&self) -> bool {
        self.tags & 32 != 0
    }
    pub fn easy(&self) -> bool {
        self.tags & 64 != 0
    }
    pub fn hard(&self) -> bool {
        self.tags & 128 != 0
    }
    pub fn tech(&self) -> bool {
        self.tags & 256 != 0
    }
    pub fn weeb(&self) -> bool {
        self.tags & 512 != 0
    }
    pub fn bluesky(&self) -> bool {
        self.tags & 1024 != 0
    }
    pub fn english(&self) -> bool {
        self.tags & 2048 != 0
    }
    pub fn tags(&self) -> Vec<MapsetTag> {
        let mut tags = Vec::with_capacity(4);
        if self.farm() {
            tags.push(MapsetTag::Farm);
        }
        if self.streams() {
            tags.push(MapsetTag::Streams);
        }
        if self.alternate() {
            tags.push(MapsetTag::Alternate);
        }
        if self.old() {
            tags.push(MapsetTag::Old);
        }
        if self.meme() {
            tags.push(MapsetTag::Meme);
        }
        if self.hardname() {
            tags.push(MapsetTag::HardName);
        }
        if self.easy() {
            tags.push(MapsetTag::Easy);
        }
        if self.hard() {
            tags.push(MapsetTag::Hard);
        }
        if self.tech() {
            tags.push(MapsetTag::Tech);
        }
        if self.weeb() {
            tags.push(MapsetTag::Weeb);
        }
        if self.bluesky() {
            tags.push(MapsetTag::BlueSky);
        }
        if self.english() {
            tags.push(MapsetTag::English);
        }
        tags
    }
    pub fn has_tag(&self, tag: MapsetTag) -> bool {
        match tag {
            MapsetTag::Farm => self.farm(),
            MapsetTag::Streams => self.streams(),
            MapsetTag::Alternate => self.alternate(),
            MapsetTag::Old => self.old(),
            MapsetTag::Meme => self.meme(),
            MapsetTag::HardName => self.hardname(),
            MapsetTag::Easy => self.easy(),
            MapsetTag::Hard => self.hard(),
            MapsetTag::Tech => self.tech(),
            MapsetTag::Weeb => self.weeb(),
            MapsetTag::BlueSky => self.bluesky(),
            MapsetTag::English => self.english(),
        }
    }
}

impl From<MapsetTagDB> for MapsetTags {
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
            + (((Some(true) == tags.english) as u32) << 11);
        Self {
            mapset_id: tags.beatmapset_id,
            mode: GameMode::from(tags.mode.unwrap()),
            filetype: tags.filetype.unwrap(),
            tags: bits,
        }
    }
}

impl fmt::Display for MapsetTags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tags = self.tags().into_iter();
        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => return Ok(()),
        };
        write!(f, "{:?}", first_tag)?;
        for tag in tags {
            write!(f, ", {:?}", tag)?;
        }
        Ok(())
    }
}
