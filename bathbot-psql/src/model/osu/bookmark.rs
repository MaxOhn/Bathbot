use rosu_v2::prelude::{GameMode, Genre, Language, RankStatus};
use time::OffsetDateTime;

pub struct MapBookmark {
    pub insert_date: OffsetDateTime,
    pub map_id: u32,
    pub mapset_id: u32,
    pub mapper_id: u32,
    pub creator_id: u32,
    pub creator_name: Box<str>,
    pub artist: Box<str>,
    pub title: Box<str>,
    pub version: Box<str>,
    pub mode: GameMode,
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub ar: f32,
    pub bpm: f32,
    pub count_circles: u32,
    pub count_sliders: u32,
    pub count_spinners: u32,
    pub seconds_drain: u32,
    pub seconds_total: u32,
    pub status: RankStatus,
    pub ranked_date: Option<OffsetDateTime>,
    pub genre: Genre,
    pub language: Language,
    pub cover_url: Box<str>,
}
