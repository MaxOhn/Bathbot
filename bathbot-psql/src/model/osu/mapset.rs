use time::OffsetDateTime;

pub struct DbBeatmapset {
    pub mapset_id: i32,
    pub user_id: i32,
    pub artist: String,
    pub title: String,
    pub creator: String,
    pub rank_status: i16,
    pub ranked_date: Option<OffsetDateTime>,
    pub thumbnail: String,
    pub cover: String,
}

pub struct ArtistTitle {
    pub artist: String,
    pub title: String,
}
