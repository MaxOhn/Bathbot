use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone)]
pub struct DbBeatmap {
    pub map_id: i32,
    pub mapset_id: i32,
    pub user_id: i32,
    pub map_version: String,
    pub seconds_drain: i32,
    pub count_circles: i32,
    pub count_sliders: i32,
    pub count_spinners: i32,
    pub bpm: f32,
}

#[derive(Debug)]
pub enum DbMapFilename {
    Present(Box<str>),
    ChecksumMismatch,
    Missing,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct MapVersion {
    pub map_id: i32,
    pub version: String,
}
