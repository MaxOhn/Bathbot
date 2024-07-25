use super::{
    list_size::ListSize, minimized_pp::MinimizedPp, score_size::ScoreSize, Authorities,
    HideSolutions, Prefixes, Retries, ScoreData,
};

pub struct DbGuildConfig {
    pub guild_id: i64,
    pub authorities: Vec<u8>,
    pub score_size: Option<i16>,
    pub list_size: Option<i16>,
    pub minimized_pp: Option<i16>,
    pub prefixes: Vec<u8>,
    pub retries: Option<i16>,
    pub osu_track_limit: Option<i16>,
    pub allow_songs: Option<bool>,
    pub render_button: Option<bool>,
    pub allow_custom_skins: Option<bool>,
    pub hide_medal_solution: Option<i16>,
    pub score_data: Option<i16>,
}

#[derive(Clone, Default)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub score_size: Option<ScoreSize>,
    pub list_size: Option<ListSize>,
    pub minimized_pp: Option<MinimizedPp>,
    pub prefixes: Prefixes,
    pub retries: Option<Retries>,
    pub track_limit: Option<u8>,
    pub allow_songs: Option<bool>,
    pub render_button: Option<bool>,
    pub allow_custom_skins: Option<bool>,
    pub hide_medal_solution: Option<HideSolutions>,
    pub score_data: Option<ScoreData>,
}

impl From<DbGuildConfig> for GuildConfig {
    #[inline]
    fn from(config: DbGuildConfig) -> Self {
        let DbGuildConfig {
            guild_id: _,
            authorities,
            score_size,
            list_size,
            minimized_pp,
            prefixes,
            retries,
            osu_track_limit,
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
            score_data,
        } = config;

        // SAFETY: The bytes originate from the DB which only provides valid archived
        // data
        let authorities = unsafe { Authorities::deserialize(&authorities) };
        let prefixes = unsafe { Prefixes::deserialize(&prefixes) };

        Self {
            authorities,
            score_size: score_size.map(ScoreSize::try_from).and_then(Result::ok),
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
            minimized_pp: minimized_pp.map(MinimizedPp::try_from).and_then(Result::ok),
            prefixes,
            retries: retries.map(Retries::try_from).and_then(Result::ok),
            track_limit: osu_track_limit.map(|limit| limit as u8),
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution: hide_medal_solution
                .map(HideSolutions::try_from)
                .and_then(Result::ok),
            score_data: score_data.map(ScoreData::try_from).and_then(Result::ok),
        }
    }
}
