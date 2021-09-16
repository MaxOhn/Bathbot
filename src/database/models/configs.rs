use crate::{commands::osu::ProfileSize, Name};

use rosu_v2::prelude::GameMode;
use serde::{Deserialize, Serialize};
use smallstr::SmallString;
use smallvec::SmallVec;
use sqlx::{postgres::PgRow, Error, FromRow, Row};

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildConfig {
    #[serde(rename = "l", alias = "with_lyrics")]
    pub with_lyrics: bool,
    #[serde(rename = "p", alias = "prefixes")]
    pub prefixes: Prefixes,
    #[serde(rename = "a", alias = "authorities")]
    pub authorities: Authorities,
}

impl<'c> FromRow<'c, PgRow> for GuildConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            with_lyrics: true,
            prefixes: smallvec!["<".into()],
            authorities: smallvec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserConfig {
    pub embeds_maximized: bool,
    pub mode: Option<GameMode>,
    pub osu_username: Option<Name>,
    pub profile_size: Option<ProfileSize>,
    pub show_retries: bool,
    pub twitch_id: Option<u64>,
}

impl<'c> FromRow<'c, PgRow> for UserConfig {
    fn from_row(row: &'c PgRow) -> Result<Self, Error> {
        let embeds_maximized = row.try_get("embeds_maximized")?;
        let mode = row.try_get::<Option<i16>, _>("mode")?;
        let osu_username = row.try_get::<Option<&'c str>, _>("osu_user_name")?;
        let profile_size = row.try_get::<Option<i16>, _>("profile_size")?;
        let show_retries = row.try_get("show_retries")?;
        let twitch_id = row.try_get::<Option<i64>, _>("twitch_id")?;

        let config = Self {
            embeds_maximized,
            mode: mode.map(|mode| mode as u8).map(GameMode::from),
            osu_username: osu_username.map(Name::from),
            profile_size: profile_size.map(ProfileSize::from),
            show_retries,
            twitch_id: twitch_id.map(|id| id as u64),
        };

        Ok(config)
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            embeds_maximized: true,
            mode: None,
            osu_username: None,
            profile_size: None,
            show_retries: true,
            twitch_id: None,
        }
    }
}
