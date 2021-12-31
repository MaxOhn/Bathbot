use crate::commands::osu::ProfileSize;

use rosu_v2::prelude::{GameMode, Username};
use smallstr::SmallString;
use smallvec::SmallVec;

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub embeds_maximized: Option<bool>,
    pub prefixes: Prefixes,
    pub profile_size: Option<ProfileSize>,
    pub show_retries: Option<bool>,
    pub with_lyrics: Option<bool>,
}

impl GuildConfig {
    pub fn with_lyrics(&self) -> bool {
        self.with_lyrics.unwrap_or(true)
    }

    pub fn embeds_maximized(&self) -> bool {
        self.embeds_maximized.unwrap_or(true)
    }

    pub fn show_retries(&self) -> bool {
        self.show_retries.unwrap_or(true)
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            authorities: SmallVec::new(),
            embeds_maximized: None,
            prefixes: smallvec!["<".into()],
            profile_size: None,
            show_retries: None,
            with_lyrics: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum OsuData {
    Name(Username),
    User { user_id: u32, username: Username },
}

impl OsuData {
    pub fn username(&self) -> &Username {
        match self {
            Self::Name(username) => username,
            Self::User { username, .. } => username,
        }
    }

    pub fn into_username(self) -> Username {
        match self {
            Self::Name(username) => username,
            Self::User { username, .. } => username,
        }
    }

    pub fn user_id(&self) -> Option<u32> {
        match self {
            Self::Name(_) => None,
            Self::User { user_id, .. } => Some(*user_id),
        }
    }
}

impl From<Username> for OsuData {
    fn from(name: Username) -> Self {
        Self::Name(name)
    }
}

impl From<String> for OsuData {
    fn from(name: String) -> Self {
        Self::Name(name.into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct UserConfig {
    pub embeds_maximized: Option<bool>,
    pub mode: Option<GameMode>,
    pub osu: Option<OsuData>,
    pub profile_size: Option<ProfileSize>,
    pub show_retries: Option<bool>,
    pub twitch_id: Option<u64>,
}

impl UserConfig {
    pub fn username(&self) -> Option<&Username> {
        self.osu.as_ref().map(OsuData::username)
    }

    pub fn into_username(self) -> Option<Username> {
        self.osu.map(OsuData::into_username)
    }
}
