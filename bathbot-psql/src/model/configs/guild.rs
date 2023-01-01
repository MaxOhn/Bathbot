use super::{
    list_size::ListSize, minimized_pp::MinimizedPp, score_size::ScoreSize, Authorities, Prefixes,
};

pub struct DbGuildConfig {
    pub guild_id: i64,
    pub authorities: Vec<u8>,
    pub score_size: Option<i16>,
    pub list_size: Option<i16>,
    pub minimized_pp: Option<i16>,
    pub prefixes: Vec<u8>,
    pub show_retries: Option<bool>,
    pub osu_track_limit: Option<i16>,
    pub allow_songs: Option<bool>,
}

#[derive(Clone, Default)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub score_size: Option<ScoreSize>,
    pub list_size: Option<ListSize>,
    pub minimized_pp: Option<MinimizedPp>,
    pub prefixes: Prefixes,
    pub show_retries: Option<bool>,
    pub track_limit: Option<u8>,
    pub allow_songs: Option<bool>,
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
            show_retries,
            osu_track_limit,
            allow_songs,
        } = config;

        // SAFETY: The bytes originate from the DB which only provides valid archived data
        let authorities = unsafe { Authorities::deserialize(&authorities) };
        let prefixes = unsafe { Prefixes::deserialize(&prefixes) };

        Self {
            authorities,
            score_size: score_size.map(ScoreSize::try_from).and_then(Result::ok),
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
            minimized_pp: minimized_pp.map(MinimizedPp::try_from).and_then(Result::ok),
            prefixes,
            show_retries,
            track_limit: osu_track_limit.map(|limit| limit as u8),
            allow_songs,
        }
    }
}
