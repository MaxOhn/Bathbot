use chrono::NaiveDateTime;
use rosu::models::{ApprovalStatus, Beatmap, GameMode, Genre, Language};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct DBMap {
    pub beatmap_id: i32,
    pub beatmapset_id: i32,
    mode: GameMode,
    version: String,
    seconds_drain: i32,
    seconds_total: i32,
    bpm: f32,
    stars: f32,
    diff_cs: f32,
    diff_od: f32,
    diff_ar: f32,
    diff_hp: f32,
    count_circle: i32,
    count_slider: i32,
    count_spinner: i32,
    max_combo: Option<i32>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DBMapSet {
    pub beatmapset_id: i32,
    pub artist: String,
    pub title: String,
    creator_id: i32,
    creator: String,
    genre: Genre,
    language: Language,
    approval_status: ApprovalStatus,
    approved_date: Option<NaiveDateTime>,
}

pub struct BeatmapWrapper(Beatmap);

impl From<Beatmap> for BeatmapWrapper {
    fn from(map: Beatmap) -> Self {
        Self(map)
    }
}

impl Into<Beatmap> for BeatmapWrapper {
    fn into(self) -> Beatmap {
        self.0
    }
}

impl<'de> Deserialize<'de> for BeatmapWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map_db = DBMap::deserialize(deserializer)?;
        let mapset_db = DBMapSet::deserialize(deserializer)?;
        let map = Beatmap::default();
        map.beatmap_id = map_db.beatmap_id as u32;
        map.beatmapset_id = mapset_db.beatmapset_id as u32;
        map.mode = map_db.mode;
        map.artist = mapset_db.artist;
        map.title = mapset_db.artist;
        map.version = map_db.version;
        map.seconds_drain = map_db.seconds_drain as u32;
        map.seconds_total = map_db.seconds_total as u32;
        map.bpm = map_db.bpm;
        map.stars = map_db.stars;
        map.diff_cs = map_db.diff_cs;
        map.diff_od = map_db.diff_od;
        map.diff_ar = map_db.diff_ar;
        map.diff_hp = map_db.diff_hp;
        map.count_circle = map_db.count_circle as u32;
        map.count_slider = map_db.count_slider as u32;
        map.count_spinner = map_db.count_spinner as u32;
        map.max_combo = map_db.max_combo.map(|combo| combo as u32);
        map.creator = mapset_db.creator;
        map.creator_id = mapset_db.creator_id as u32;
        map.genre = mapset_db.genre;
        map.language = mapset_db.language;
        map.approval_status = mapset_db.approval_status;
        map.approved_date = mapset_db.approved_date;
        Ok(Self(map))
    }
}
