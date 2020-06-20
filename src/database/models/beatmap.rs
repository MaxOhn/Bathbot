use chrono::NaiveDateTime;
use rosu::models::Beatmap;
use sqlx::{mysql::MySqlRow, FromRow, Row};

#[derive(FromRow, Debug)]
pub struct DBMap {
    pub beatmap_id: u32,
    pub beatmapset_id: u32,
    mode: u8,
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

impl<'c> FromRow<'c, MySqlRow> for BeatmapWrapper {
    fn from_row(row: &MySqlRow) -> Result<BeatmapWrapper, sqlx::Error> {
        let mode: u8 = row.get("mode");
        let genre: u8 = row.get("genre");
        let language: u8 = row.get("language");
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
        map.mode = mode.into();
        map.max_combo = row.get("max_combo");
        map.genre = genre.into();
        map.language = language.into();
        map.approval_status = status.into();
        map.approved_date = row.get("approved_date");
        Ok(Self(map))
    }
}
