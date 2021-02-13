use crate::unwind_error;

use chrono::{DateTime, Utc};
use rosu::model::{ApprovalStatus, Beatmap};
use sqlx::{postgres::PgRow, FromRow, Row};
use std::convert::TryInto;

#[derive(Debug, FromRow)]
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
    genre: i8,
    language: i8,
    approval_status: i8,
    approved_date: Option<DateTime<Utc>>,
}

pub struct BeatmapWrapper(Beatmap);

impl From<Beatmap> for BeatmapWrapper {
    #[inline]
    fn from(map: Beatmap) -> Self {
        Self(map)
    }
}

impl Into<Beatmap> for BeatmapWrapper {
    #[inline]
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

        let map = Beatmap {
            beatmap_id: row.get("beatmap_id"),
            beatmapset_id: row.get("beatmapset_id"),
            version: row.get("version"),
            seconds_drain: row.get("seconds_drain"),
            seconds_total: row.get("seconds_total"),
            bpm: row.get("bpm"),
            stars: row.get("stars"),
            diff_cs: row.get("diff_cs"),
            diff_ar: row.get("diff_ar"),
            diff_od: row.get("diff_od"),
            diff_hp: row.get("diff_hp"),
            count_circle: row.get("count_circle"),
            count_slider: row.get("count_slider"),
            count_spinner: row.get("count_spinner"),
            artist: row.get("artist"),
            title: row.get("title"),
            creator_id: row.get("creator_id"),
            creator: row.get("creator"),
            mode: (mode as u8).into(),
            max_combo: row.get("max_combo"),
            genre: (genre as u8).into(),
            language: (language as u8).into(),
            approval_status: match status.try_into() {
                Ok(status) => status,
                Err(why) => {
                    unwind_error!(warn, why, "{}");
                    ApprovalStatus::WIP
                }
            },
            approved_date: row.get("approved_date"),
            ..Default::default()
        };

        Ok(Self(map))
    }
}
