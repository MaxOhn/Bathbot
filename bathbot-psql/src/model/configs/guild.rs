use sqlx::types::JsonValue;

use super::{Authorities, HideSolutions, Retries, ScoreData, list_size::ListSize};

pub struct DbGuildConfig {
    pub guild_id: i64,
    pub authorities: Vec<u8>,
    pub list_size: Option<i16>,
    pub prefixes: JsonValue,
    pub retries: Option<i16>,
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
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
            score_data,
        } = config;

        let authorities = Authorities::deserialize(&authorities);

        let JsonValue::Array(array) = prefixes else {
            unreachable!()
        };

        let prefixes = array
            .into_iter()
            .map(|value| match value {
                JsonValue::String(prefix) => prefix,
                _ => unreachable!(),
            })
            .collect();

        Self {
            authorities,
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
            prefixes,
            retries: retries.map(Retries::try_from).and_then(Result::ok),
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
