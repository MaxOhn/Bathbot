use super::super::schema::beatmaps;
use chrono::{DateTime, NaiveDateTime, Utc};
use rosu::models::{ApprovalStatus, Beatmap, GameMode, Genre, Language};
use std::convert::TryFrom;

#[derive(Queryable, Insertable)]
#[table_name = "beatmaps"]
pub struct DBBeatmap {
    pub beatmap_id: u32,
    pub beatmapset_id: u32,
    pub mode: u8,
    pub artist: String,
    pub title: String,
    pub version: String,
    pub creator_id: u32,
    pub creator: String,
    pub seconds_drain: u32,
    pub seconds_total: u32,
    pub bpm: f32,
    pub stars: f32,
    pub diff_cs: f32,
    pub diff_od: f32,
    pub diff_ar: f32,
    pub diff_hp: f32,
    pub count_circle: u32,
    pub count_slider: u32,
    pub count_spinner: u32,
    pub max_combo: Option<u32>,
    pub genre: u8,
    pub language: u8,
    pub approval_status: i8,
    pub approved_date: Option<NaiveDateTime>,
}

impl From<&Beatmap> for DBBeatmap {
    fn from(map: &Beatmap) -> Self {
        Self {
            beatmap_id: map.beatmap_id,
            beatmapset_id: map.beatmapset_id,
            mode: map.mode as u8,
            artist: map.artist.to_owned(),
            title: map.title.to_owned(),
            version: map.version.to_owned(),
            creator_id: map.creator_id,
            creator: map.creator.to_owned(),
            seconds_drain: map.seconds_drain,
            seconds_total: map.seconds_total,
            bpm: map.bpm,
            stars: map.stars,
            diff_cs: map.diff_cs,
            diff_od: map.diff_od,
            diff_ar: map.diff_ar,
            diff_hp: map.diff_hp,
            count_circle: map.count_circle,
            count_slider: map.count_slider,
            count_spinner: map.count_spinner,
            max_combo: map.max_combo,
            genre: map.genre as u8,
            language: map.language as u8,
            approval_status: map.approval_status as i8,
            approved_date: Some(map.approved_date.as_ref().unwrap().naive_utc()),
        }
    }
}

impl Into<Beatmap> for DBBeatmap {
    fn into(self) -> Beatmap {
        let mut map = Beatmap::default();
        map.beatmap_id = self.beatmap_id;
        map.beatmapset_id = self.beatmapset_id;
        map.artist = self.artist;
        map.title = self.title;
        map.version = self.version;
        map.mode = GameMode::try_from(self.mode)
            .unwrap_or_else(|e| panic!("Error parsing GameMode: {}", e));
        map.creator = self.creator;
        map.creator_id = self.creator_id;
        map.seconds_drain = self.seconds_drain;
        map.seconds_total = self.seconds_total;
        map.bpm = self.bpm;
        map.stars = self.stars;
        map.diff_cs = self.diff_cs;
        map.diff_ar = self.diff_ar;
        map.diff_hp = self.diff_hp;
        map.diff_od = self.diff_od;
        map.count_circle = self.count_circle;
        map.count_slider = self.count_slider;
        map.count_spinner = self.count_spinner;
        map.max_combo = self.max_combo;
        map.genre =
            Genre::try_from(self.genre).unwrap_or_else(|e| panic!("Error parsing Genre: {}", e));
        map.language = Language::try_from(self.language)
            .unwrap_or_else(|e| panic!("Error parsing Language: {}", e));
        map.approval_status = ApprovalStatus::try_from(self.approval_status)
            .unwrap_or_else(|e| panic!("Error parsing ApprovalStatus: {}", e));
        map.approved_date = self.approved_date.map(|date| DateTime::from_utc(date, Utc));
        map
    }
}
