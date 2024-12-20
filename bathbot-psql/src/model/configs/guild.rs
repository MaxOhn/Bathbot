use sqlx::types::JsonValue;

use super::{list_size::ListSize, Authorities, HideSolutions, Retries, ScoreData};

pub struct DbGuildConfig {
    pub guild_id: i64,
    pub authorities: Vec<u8>,
    pub list_size: Option<i16>,
    pub prefixes: JsonValue,
    pub retries: Option<i16>,
    pub osu_track_limit: Option<i16>,
    pub allow_songs: Option<bool>,
    pub render_button: Option<bool>,
    pub allow_custom_skins: Option<bool>,
    pub hide_medal_solution: Option<i16>,
    pub score_data: Option<i16>,
}

#[derive(Clone)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub list_size: Option<ListSize>,
    pub prefixes: Vec<String>,
    pub retries: Option<Retries>,
    pub track_limit: Option<u8>,
    pub allow_songs: Option<bool>,
    pub render_button: Option<bool>,
    pub allow_custom_skins: Option<bool>,
    pub hide_medal_solution: Option<HideSolutions>,
    pub score_data: Option<ScoreData>,
}

impl GuildConfig {
    pub const DEFAULT_PREFIX: &str = "<";
}

impl Default for GuildConfig {
    fn default() -> Self {
        Self {
            authorities: Default::default(),
            list_size: Default::default(),
            prefixes: vec![Self::DEFAULT_PREFIX.to_owned()],
            retries: Default::default(),
            track_limit: Default::default(),
            allow_songs: Default::default(),
            render_button: Default::default(),
            allow_custom_skins: Default::default(),
            hide_medal_solution: Default::default(),
            score_data: Default::default(),
        }
    }
}

impl From<DbGuildConfig> for GuildConfig {
    #[inline]
    fn from(config: DbGuildConfig) -> Self {
        let DbGuildConfig {
            guild_id: _,
            authorities,
            list_size,
            prefixes,
            retries,
            osu_track_limit,
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
            score_data,
        } = config;

        let authorities = Authorities::deserialize(&authorities);

        let prefixes = if let JsonValue::Array(array) = prefixes {
            array
                .into_iter()
                .map(|value| match value {
                    JsonValue::String(prefix) => prefix,
                    _ => unreachable!(),
                })
                .collect()
        } else {
            unreachable!()
        };

        Self {
            authorities,
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
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
