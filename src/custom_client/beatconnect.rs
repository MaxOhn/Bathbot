use rosu_v2::model::GameMode;
use serde::Deserialize;
use std::fmt;

pub struct BeatconnectSearchParams {
    pub query: String,
    pub page: usize,
    mode: Option<GameMode>,
    status: BeatconnectMapStatus,
}

impl BeatconnectSearchParams {
    #[inline]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            page: 0,
            mode: None,
            status: BeatconnectMapStatus::Ranked,
        }
    }

    #[inline]
    pub fn next_page(&mut self) {
        self.page += 1;
    }

    #[inline]
    pub fn mode(&mut self, mode: GameMode) {
        self.mode.replace(mode);
    }

    #[inline]
    pub fn status(&mut self, status: BeatconnectMapStatus) {
        self.status = status;
    }
}

impl fmt::Display for BeatconnectSearchParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(mode) = self.mode {
            write!(f, "m={}&", mode)?;
        }

        write!(
            f,
            "s={status}&p={page}&q={query}",
            status = self.status,
            page = self.page,
            query = self.query
        )
    }
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeatconnectMapStatus {
    All,
    Approved,
    Graveyard,
    Loved,
    Pending,
    Qualified,
    Ranked,
    Unranked,
    WIP,
}

impl fmt::Display for BeatconnectMapStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::All => "all",
            Self::Approved => "approved",
            Self::Graveyard => "graveyard",
            Self::Loved => "loved",
            Self::Pending => "pending",
            Self::Qualified => "qualified",
            Self::Ranked => "ranked",
            Self::Unranked => "unranked",
            Self::WIP => "wip",
        };

        f.write_str(s)
    }
}

#[derive(Debug, Deserialize)]
pub struct BeatconnectSearchResponse {
    #[serde(rename = "beatmaps")]
    pub mapsets: Vec<BeatconnectMapSet>,
    max_page: usize,
}

impl BeatconnectSearchResponse {
    #[inline]
    pub fn is_last_page(&self) -> bool {
        self.max_page == 0
    }
}

#[derive(Debug, Deserialize)]
pub struct BeatconnectMapSet {
    #[serde(rename = "id")]
    pub beatmapset_id: u32,
    pub title: String,
    pub artist: String,
    pub creator: String,
    #[serde(rename = "user_id")]
    pub creator_id: u32,
    pub bpm: f32,
    pub status: BeatconnectMapStatus,
    #[serde(rename = "beatmaps")]
    pub maps: Vec<BeatconnectMap>,
    pub mode_std: bool,
    pub mode_mania: bool,
    pub mode_taiko: bool,
    pub mode_ctb: bool,
    // has_scores: bool,
    // submitted_date: DateTime<Utc>,
    // last_updated: DateTime<Utc>,
    // ranked_date: DateTime<Utc>,
    // has_favourited: bool,
    // #[serde(rename = "average_length")]
    // avg_len: u32,
    // source: String,
    // covers_id: u32,
    // tags: String,
    // video: bool,
    // storyboard: bool,
    // discussion_enabled: bool,
    // ranked: u8,
    // legacy_thread_url: String,
    // preview_url: String,
    // unique_id: String,
    // #[serde(default)]
    // map_not_found: bool,
}

#[derive(Debug, Deserialize)]
pub struct BeatconnectMap {
    #[serde(rename = "id")]
    pub beatmap_id: u32,
    pub mode: GameMode,
    #[serde(rename = "difficulty")]
    pub stars: f32,
    pub version: String,
    #[serde(rename = "total_length")]
    pub seconds_total: u32,
    pub cs: f32,
    #[serde(rename = "drain")]
    pub hp: f32,
    #[serde(rename = "accuracy")]
    pub od: f32,
    pub ar: f32,
    pub count_circles: usize,
    pub count_sliders: usize,
    pub count_spinners: usize,
    pub status: BeatconnectMapStatus,
    // count_total: Option<usize>,
    // last_updated: i64,
    // url: String,
}
