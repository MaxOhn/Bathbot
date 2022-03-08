use crate::commands::osu::ProfileSize;

use rosu_v2::prelude::{GameMode, Username};
use smallstr::SmallString;
use smallvec::SmallVec;

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EmbedsSize {
    AlwaysMinimized = 0,
    InitialMaximized = 1,
    AlwaysMaximized = 2,
}

impl From<i16> for EmbedsSize {
    fn from(value: i16) -> Self {
        match value {
            0 => Self::AlwaysMinimized,
            2 => Self::AlwaysMaximized,
            _ => Self::InitialMaximized,
        }
    }
}

impl Default for EmbedsSize {
    fn default() -> Self {
        Self::InitialMaximized
    }
}

#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub embeds_size: Option<EmbedsSize>,
    pub prefixes: Prefixes,
    pub profile_size: Option<ProfileSize>,
    pub show_retries: Option<bool>,
    pub track_limit: Option<u8>,
    pub with_lyrics: Option<bool>,
}

impl GuildConfig {
    pub fn with_lyrics(&self) -> bool {
        self.with_lyrics.unwrap_or(true)
    }

    pub fn embeds_size(&self) -> EmbedsSize {
        self.embeds_size.unwrap_or_default()
    }

    pub fn show_retries(&self) -> bool {
        self.show_retries.unwrap_or(true)
    }

    pub fn track_limit(&self) -> u8 {
        self.track_limit.unwrap_or(50)
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            authorities: SmallVec::new(),
            embeds_size: None,
            prefixes: smallvec!["<".into()],
            profile_size: None,
            show_retries: None,
            track_limit: None,
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
    pub embeds_size: Option<EmbedsSize>,
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

    pub fn embeds_size(&self) -> EmbedsSize {
        self.embeds_size.unwrap_or_default()
    }
}
