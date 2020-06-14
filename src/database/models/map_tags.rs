use super::{super::schema::map_tags, beatmap::DBMapSet};
use crate::commands::fun::MapsetTag;

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
}
