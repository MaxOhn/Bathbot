use super::{super::schema::map_tags, beatmap::DBMapSet};
use crate::commands::utility::MapsetTag;

use std::fmt::{self, Write};

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
    pub fn any(&self) -> bool {
        self.farm == Some(true)
            || self.streams == Some(true)
            || self.alternate == Some(true)
            || self.old == Some(true)
            || self.meme == Some(true)
            || self.hardname == Some(true)
            || self.easy == Some(true)
            || self.hard == Some(true)
            || self.tech == Some(true)
            || self.weeb == Some(true)
            || self.bluesky == Some(true)
            || self.english == Some(true)
    }
    pub fn untagged(&self) -> bool {
        !self.any()
    }
}

impl fmt::Display for MapsetTagDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.any() {
            let mut buf = String::with_capacity(32);
            if let Some(true) = self.farm {
                write!(buf, "farm, ")?;
            }
            if let Some(true) = self.streams {
                write!(buf, "streams, ")?;
            }
            if let Some(true) = self.alternate {
                write!(buf, "alternate, ")?;
            }
            if let Some(true) = self.old {
                write!(buf, "old, ")?;
            }
            if let Some(true) = self.meme {
                write!(buf, "meme, ")?;
            }
            if let Some(true) = self.hardname {
                write!(buf, "hardname, ")?;
            }
            if let Some(true) = self.easy {
                write!(buf, "easy, ")?;
            }
            if let Some(true) = self.hard {
                write!(buf, "hard, ")?;
            }
            if let Some(true) = self.tech {
                write!(buf, "tech, ")?;
            }
            if let Some(true) = self.weeb {
                write!(buf, "weeb, ")?;
            }
            if let Some(true) = self.bluesky {
                write!(buf, "bluesky, ")?;
            }
            if let Some(true) = self.english {
                write!(buf, "english, ")?;
            }
            buf.pop();
            buf.pop();
            write!(f, "{}", buf)?;
        } else {
            write!(f, "None")?;
        }
        Ok(())
    }
}
