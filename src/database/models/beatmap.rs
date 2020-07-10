// use crate::database::util::serde_maybe_date;

// use chrono::{offset::TimeZone, DateTime, Utc};
// use rosu::models::{ApprovalStatus, Beatmap, GameMode, Genre, Language};
// use serde::{Deserialize, Serialize};
// use tokio_postgres::Row;

// #[derive(Deserialize, Serialize, Debug)]
// pub struct DBMap {
//     pub beatmap_id: i32,
//     pub beatmapset_id: i32,
//     mode: GameMode,
//     version: String,
//     seconds_drain: i32,
//     seconds_total: i32,
//     bpm: f32,
//     stars: f32,
//     diff_cs: f32,
//     diff_od: f32,
//     diff_ar: f32,
//     diff_hp: f32,
//     count_circle: i32,
//     count_slider: i32,
//     count_spinner: i32,
//     max_combo: Option<i32>,
// }

// #[derive(Deserialize, Serialize, Debug)]
// pub struct DBMapSet {
//     pub beatmapset_id: i32,
//     pub artist: String,
//     pub title: String,
//     creator_id: i32,
//     creator: String,
//     genre: Genre,
//     language: Language,
//     approval_status: ApprovalStatus,
//     #[serde(with = "serde_maybe_date")]
//     approved_date: Option<DateTime<Utc>>,
// }

// pub struct BeatmapWrapper(Beatmap);

// impl From<Beatmap> for BeatmapWrapper {
//     fn from(map: Beatmap) -> Self {
//         Self(map)
//     }
// }

// impl Into<Beatmap> for BeatmapWrapper {
//     fn into(self) -> Beatmap {
//         self.0
//     }
// }

// impl From<Row> for BeatmapWrapper {
//     fn from(row: Row) -> Self {
//         let mut map = Beatmap::default();
//         map.beatmap_id = row.get("beatmap_id");
//         map.beatmapset_id = row.get("beatmapset_id");
//         let mode: i8 = row.get("mode");
//         map.mode = GameMode::from(mode as u8);
//         map.artist = row.get("artist");
//         map.title = row.get("title");
//         map.version = row.get("version");
//         map.seconds_drain = row.get("seconds_drain");
//         map.seconds_total = row.get("seconds_total");
//         map.bpm = row.get("bpm");
//         map.stars = row.get("stars");
//         map.diff_cs = row.get("diff_cs");
//         map.diff_od = row.get("diff_od");
//         map.diff_ar = row.get("diff_ar");
//         map.diff_hp = row.get("diff_hp");
//         map.count_circle = row.get("count_circle");
//         map.count_slider = row.get("count_slider");
//         map.count_spinner = row.get("count_spinner");
//         map.max_combo = row.get("max_combo");
//         map.creator = row.get("creator");
//         map.creator_id = row.get("creator_id");
//         map.genre = Genre::from(row.get::<_, i8>("genre") as u8);
//         map.language = Language::from(row.get::<_, i8>("language") as u8);
//         map.approval_status = ApprovalStatus::from(row.get::<_, i8>("approval_status"));
//         let date: String = row.get("approved_date");
//         map.approved_date = Utc.datetime_from_str(&date, "%F %T").ok();
//         Self(map)
//     }
// }

// impl<'de> Deserialize<'de> for BeatmapWrapper {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let map_db = DBMap::deserialize(deserializer)?;
//         let mapset_db = DBMapSet::deserialize(deserializer)?;
//         let map = Beatmap::default();
//         map.beatmap_id = map_db.beatmap_id as u32;
//         map.beatmapset_id = mapset_db.beatmapset_id as u32;
//         map.mode = map_db.mode;
//         map.artist = mapset_db.artist;
//         map.title = mapset_db.title;
//         map.version = map_db.version;
//         map.seconds_drain = map_db.seconds_drain as u32;
//         map.seconds_total = map_db.seconds_total as u32;
//         map.bpm = map_db.bpm;
//         map.stars = map_db.stars;
//         map.diff_cs = map_db.diff_cs;
//         map.diff_od = map_db.diff_od;
//         map.diff_ar = map_db.diff_ar;
//         map.diff_hp = map_db.diff_hp;
//         map.count_circle = map_db.count_circle as u32;
//         map.count_slider = map_db.count_slider as u32;
//         map.count_spinner = map_db.count_spinner as u32;
//         map.max_combo = map_db.max_combo.map(|combo| combo as u32);
//         map.creator = mapset_db.creator;
//         map.creator_id = mapset_db.creator_id as u32;
//         map.genre = mapset_db.genre;
//         map.language = mapset_db.language;
//         map.approval_status = mapset_db.approval_status;
//         map.approved_date = mapset_db.approved_date;
//         Ok(Self(map))
//     }
// }

use chrono::{DateTime, Utc};
use rosu::models::Beatmap;
use sqlx::{postgres::PgRow, FromRow, Row};

#[derive(FromRow, Debug)]
pub struct DBMap {
    pub beatmap_id: u32,
    pub beatmapset_id: u32,
    mode: i8,
    version: String,
    seconds_drain: u32,
    seconds_total: u32,
    bpm: f32,
    stars: f32,
    diff_cs: f32,
    diff_od: f32,
    diff_ar: f32,
    diff_hp: f32,
    count_circle: u32,
    count_slider: u32,
    count_spinner: u32,
    max_combo: Option<u32>,
}

#[derive(FromRow, Debug)]
pub struct DBMapSet {
    pub beatmapset_id: u32,
    pub artist: String,
    pub title: String,
    creator_id: u32,
    creator: String,
    genre: u8,
    language: u8,
    approval_status: i8,
    approved_date: Option<DateTime<Utc>>,
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

impl<'c> FromRow<'c, PgRow> for BeatmapWrapper {
    fn from_row(row: &PgRow) -> Result<BeatmapWrapper, sqlx::Error> {
        let mode: i8 = row.get("mode");
        let genre: i8 = row.get("genre");
        let language: i8 = row.get("language");
        let status: i8 = row.get("approval_status");
        let mut map = Beatmap::default();
        map.beatmap_id = row.get("beatmap_id");
        map.beatmapset_id = row.get("beatmapset_id");
        map.version = row.get("version");
        map.seconds_drain = row.get("seconds_drain");
        map.seconds_total = row.get("seconds_total");
        map.bpm = row.get("bpm");
        map.stars = row.get("stars");
        map.diff_cs = row.get("diff_cs");
        map.diff_ar = row.get("diff_ar");
        map.diff_od = row.get("diff_od");
        map.diff_hp = row.get("diff_hp");
        map.count_circle = row.get("count_circle");
        map.count_slider = row.get("count_slider");
        map.count_spinner = row.get("count_spinner");
        map.artist = row.get("artist");
        map.title = row.get("title");
        map.creator_id = row.get("creator_id");
        map.creator = row.get("creator");
        map.mode = (mode as u8).into();
        map.max_combo = row.get("max_combo");
        map.genre = (genre as u8).into();
        map.language = (language as u8).into();
        map.approval_status = status.into();
        map.approved_date = row.get("approved_date");
        Ok(Self(map))
    }
}
