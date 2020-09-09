use super::deserialize::adjust_mode;

use rosu::models::GameMode;
use serde::{Deserialize, Deserializer};
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub struct MostPlayedMap {
    pub beatmap_id: u32,
    pub count: u32,
    pub mode: GameMode,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub stars: f32,
}

impl Hash for MostPlayedMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.beatmap_id.hash(state);
    }
}

impl PartialEq for MostPlayedMap {
    fn eq(&self, other: &Self) -> bool {
        self.beatmap_id == other.beatmap_id
    }
}

impl Eq for MostPlayedMap {}

impl<'de> Deserialize<'de> for MostPlayedMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Outer {
            beatmap_id: u32,
            count: u32,
            beatmap: InnerMap,
            beatmapset: InnerMapset,
        }

        #[derive(Deserialize)]
        pub struct InnerMap {
            #[serde(deserialize_with = "adjust_mode")]
            mode: GameMode,
            version: String,
            difficulty_rating: f32,
        }

        #[derive(Deserialize)]
        pub struct InnerMapset {
            title: String,
            artist: String,
        }

        let helper = Outer::deserialize(d)?;
        Ok(MostPlayedMap {
            beatmap_id: helper.beatmap_id,
            count: helper.count,
            mode: helper.beatmap.mode,
            title: helper.beatmapset.title,
            artist: helper.beatmapset.artist,
            version: helper.beatmap.version,
            stars: helper.beatmap.difficulty_rating,
        })
    }
}
