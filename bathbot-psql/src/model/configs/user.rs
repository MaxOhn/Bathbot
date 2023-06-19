use rosu_v2::prelude::{GameMode, Username};
use time::UtcOffset;

use super::{list_size::ListSize, minimized_pp::MinimizedPp, score_size::ScoreSize};

pub struct DbUserConfig {
    pub score_size: Option<i16>,
    pub list_size: Option<i16>,
    pub minimized_pp: Option<i16>,
    pub gamemode: Option<i16>,
    pub osu_id: Option<i32>,
    pub show_retries: Option<bool>,
    pub twitch_id: Option<i64>,
    pub timezone_seconds: Option<i32>,
    pub render_button: Option<bool>,
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
    pub score_size: Option<ScoreSize>,
    pub list_size: Option<ListSize>,
    pub minimized_pp: Option<MinimizedPp>,
    pub mode: Option<GameMode>,
    pub osu: Option<O::Type>,
    pub show_retries: Option<bool>,
    pub twitch_id: Option<u64>,
    pub timezone: Option<UtcOffset>,
    pub render_button: Option<bool>,
}

impl<O: OsuId> Default for UserConfig<O> {
    #[inline]
    fn default() -> Self {
        Self {
            score_size: None,
            list_size: None,
            minimized_pp: None,
            mode: None,
            osu: None,
            show_retries: None,
            twitch_id: None,
            timezone: None,
            render_button: None,
        }
    }
}

impl From<DbUserConfig> for UserConfig<OsuUserId> {
    #[inline]
    fn from(config: DbUserConfig) -> Self {
        let DbUserConfig {
            score_size,
            list_size,
            minimized_pp,
            gamemode,
            osu_id,
            show_retries,
            twitch_id,
            timezone_seconds,
            render_button,
        } = config;

        Self {
            score_size: score_size.map(ScoreSize::try_from).and_then(Result::ok),
            list_size: list_size.map(ListSize::try_from).and_then(Result::ok),
            minimized_pp: minimized_pp.map(MinimizedPp::try_from).and_then(Result::ok),
            mode: gamemode.map(|mode| GameMode::from(mode as u8)),
            osu: osu_id.map(|id| id as u32),
            show_retries,
            twitch_id: twitch_id.map(|id| id as u64),
            timezone: timezone_seconds
                .map(UtcOffset::from_whole_seconds)
                .map(Result::unwrap),
            render_button,
        }
    }
}
