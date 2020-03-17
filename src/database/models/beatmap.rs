use super::super::schema::{maps, mapsets};
use chrono::{DateTime, NaiveDateTime, Utc};
use rosu::models::{ApprovalStatus, Beatmap, GameMode, Genre, Language};
use std::convert::TryFrom;

pub trait MapSplit {
    fn db_split(&self) -> (DBMap, DBMapSet);
    fn into_db_split(self) -> (DBMap, DBMapSet);
}

impl MapSplit for Beatmap {
    fn db_split(&self) -> (DBMap, DBMapSet) {
        let map = DBMap {
            beatmap_id: self.beatmap_id,
            beatmapset_id: self.beatmapset_id,
            mode: self.mode as u8,
            version: self.version.to_owned(),
            seconds_drain: self.seconds_drain,
            seconds_total: self.seconds_total,
            bpm: self.bpm,
            stars: self.stars,
            diff_cs: self.diff_cs,
            diff_od: self.diff_od,
            diff_ar: self.diff_ar,
            diff_hp: self.diff_hp,
            count_circle: self.count_circle,
            count_slider: self.count_slider,
            count_spinner: self.count_spinner,
            max_combo: self.max_combo,
        };
        let mapset = DBMapSet {
            beatmapset_id: self.beatmapset_id,
            artist: self.artist.to_owned(),
            title: self.title.to_owned(),
            creator_id: self.creator_id,
            creator: self.creator.to_owned(),
            genre: self.genre as u8,
            language: self.language as u8,
            approval_status: self.approval_status as i8,
            approved_date: Some(self.approved_date.as_ref().unwrap().naive_utc()),
        };
        (map, mapset)
    }

    fn into_db_split(self) -> (DBMap, DBMapSet) {
        let map = DBMap {
            beatmap_id: self.beatmap_id,
            beatmapset_id: self.beatmapset_id,
            mode: self.mode as u8,
            version: self.version,
            seconds_drain: self.seconds_drain,
            seconds_total: self.seconds_total,
            bpm: self.bpm,
            stars: self.stars,
            diff_cs: self.diff_cs,
            diff_od: self.diff_od,
            diff_ar: self.diff_ar,
            diff_hp: self.diff_hp,
            count_circle: self.count_circle,
            count_slider: self.count_slider,
            count_spinner: self.count_spinner,
            max_combo: self.max_combo,
        };
        let mapset = DBMapSet {
            beatmapset_id: self.beatmapset_id,
            artist: self.artist,
            title: self.title,
            creator_id: self.creator_id,
            creator: self.creator,
            genre: self.genre as u8,
            language: self.language as u8,
            approval_status: self.approval_status as i8,
            approved_date: Some(self.approved_date.as_ref().unwrap().naive_utc()),
        };
        (map, mapset)
    }
}

#[derive(Identifiable, Debug, Queryable, Insertable, Associations)]
#[table_name = "maps"]
#[belongs_to(DBMapSet, foreign_key = "beatmapset_id")]
#[primary_key(beatmap_id)]
pub struct DBMap {
    pub beatmap_id: u32, // pub for debugging purposes
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

impl DBMap {
    pub fn into_beatmap(self, mapset: DBMapSet) -> Beatmap {
        let mut map = Beatmap::default();
        map.beatmap_id = self.beatmap_id;
        map.beatmapset_id = self.beatmapset_id;
        map.artist = mapset.artist;
        map.title = mapset.title;
        map.version = self.version;
        map.mode = GameMode::try_from(self.mode)
            .unwrap_or_else(|e| panic!("Error parsing GameMode: {}", e));
        map.creator = mapset.creator;
        map.creator_id = mapset.creator_id;
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
            Genre::try_from(mapset.genre).unwrap_or_else(|e| panic!("Error parsing Genre: {}", e));
        map.language = Language::try_from(mapset.language)
            .unwrap_or_else(|e| panic!("Error parsing Language: {}", e));
        map.approval_status = ApprovalStatus::try_from(mapset.approval_status)
            .unwrap_or_else(|e| panic!("Error parsing ApprovalStatus: {}", e));
        map.approved_date = mapset
            .approved_date
            .map(|date| DateTime::from_utc(date, Utc));
        map
    }
}

#[derive(Identifiable, Debug, Queryable, Insertable, Associations, Clone)]
#[table_name = "mapsets"]
#[primary_key(beatmapset_id)]
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
