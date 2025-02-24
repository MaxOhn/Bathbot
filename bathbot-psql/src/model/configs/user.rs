use bathbot_model::embed_builder::ScoreEmbedSettings;
use rosu_v2::prelude::{GameMode, Username};
use sqlx::types::Json;
use time::UtcOffset;

use super::{Retries, ScoreData, list_size::ListSize};

pub struct DbUserConfig {
    pub list_size: Option<i16>,
    pub score_embed: Option<Json<ScoreEmbedSettings>>,
    pub gamemode: Option<i16>,
    pub osu_id: Option<i32>,
    pub retries: Option<i16>,
    pub twitch_id: Option<i64>,
    pub timezone_seconds: Option<i32>,
    pub render_button: Option<bool>,
    pub score_data: Option<i16>,
}

pub trait OsuId {
    type Type;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OsuUserId;

impl OsuId for OsuUserId {
    type Type = u32;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OsuUsername;

impl OsuId for OsuUsername {
    type Type = Username;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserConfig<O: OsuId> {
    pub list_size: Option<ListSize>,
    pub score_embed: Option<ScoreEmbedSettings>,
    pub mode: Option<GameMode>,
    pub osu: Option<O::Type>,
    pub retries: Option<Retries>,
    pub twitch_id: Option<u64>,
    pub timezone: Option<UtcOffset>,
    pub render_button: Option<bool>,
    pub score_data: Option<ScoreData>,
}

impl<O: OsuId> Default for UserConfig<O> {
    #[inline]
    fn default() -> Self {
        Self {
            list_size: None,
            score_embed: None,
            mode: None,
            osu: None,
            retries: None,
            twitch_id: None,
            timezone: None,
            render_button: None,
            score_data: None,
        }
    }
}

impl From<DbUserConfig> for UserConfig<OsuUserId> {
    #[inline]
    fn from(config: DbUserConfig) -> Self {
        let DbUserConfig {
            list_size,
            score_embed,
            gamemode,
            osu_id,
            retries,
            twitch_id,
            timezone_seconds,
            render_button,
            score_data,
        } = config;

        Self {
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
            score_embed: score_embed.map(|Json(score_embed)| score_embed),
            mode: gamemode.map(|mode| GameMode::from(mode as u8)),
            osu: osu_id.map(|id| id as u32),
            retries: retries.map(Retries::try_from).and_then(Result::ok),
            twitch_id: twitch_id.map(|id| id as u64),
            timezone: timezone_seconds
                .map(UtcOffset::from_whole_seconds)
                .map(Result::unwrap),
            render_button,
            score_data: score_data.map(ScoreData::try_from).and_then(Result::ok),
        }
    }
}
