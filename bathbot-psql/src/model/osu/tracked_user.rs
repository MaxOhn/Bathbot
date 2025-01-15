pub struct DbTrackedOsuUser {
    pub user_id: i32,
    pub gamemode: i16,
    pub channel_id: i64,
    pub min_index: Option<i16>,
    pub max_index: Option<i16>,
    pub min_pp: Option<f32>,
    pub max_pp: Option<f32>,
    pub min_combo_percent: Option<f32>,
    pub max_combo_percent: Option<f32>,
    pub last_pp: f32,
}

pub struct DbTrackedOsuUserInChannel {
    pub user_id: i32,
    pub gamemode: i16,
    pub min_index: Option<i16>,
    pub max_index: Option<i16>,
    pub min_pp: Option<f32>,
    pub max_pp: Option<f32>,
    pub min_combo_percent: Option<f32>,
    pub max_combo_percent: Option<f32>,
}
