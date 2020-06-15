use super::{super::schema::map_tags, beatmap::DBMapSet};
use crate::commands::fun::MapsetTag;

use std::fmt::{self, Write};

#[derive(Default, Copy, Clone, Identifiable, Queryable, Associations, Insertable, AsChangeset)]
#[table_name = "map_tags"]
#[belongs_to(DBMapSet, foreign_key = "beatmapset_id")]
#[primary_key(beatmapset_id)]
pub struct MapsetTagDB {
    beatmapset_id: u32,
    farm: Option<bool>,
    streams: Option<bool>,
    alternate: Option<bool>,
    old: Option<bool>,
    meme: Option<bool>,
    hardname: Option<bool>,
    easy: Option<bool>,
    hard: Option<bool>,
    tech: Option<bool>,
    bluesky: Option<bool>,
    weeb: Option<bool>,
    english: Option<bool>,
}

impl MapsetTagDB {
    pub fn new(beatmapset_id: u32) -> Self {
        Self {
            beatmapset_id,
            farm: None,
            streams: None,
            alternate: None,
            old: None,
            meme: None,
            hardname: None,
            easy: None,
            hard: None,
            tech: None,
            bluesky: None,
            weeb: None,
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
    fn any(&self) -> bool {
        self.farm.is_some()
            | self.streams.is_some()
            | self.alternate.is_some()
            | self.old.is_some()
            | self.meme.is_some()
            | self.hardname.is_some()
            | self.easy.is_some()
            | self.hard.is_some()
            | self.tech.is_some()
            | self.weeb.is_some()
            | self.bluesky.is_some()
            | self.english.is_some()
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
