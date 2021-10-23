use crate::{commands::osu::ProfileSize, Context, Name};

use rosu_v2::prelude::{GameMode, User};
use smallstr::SmallString;
use smallvec::SmallVec;

pub type Prefix = SmallString<[u8; 2]>;
pub type Prefixes = SmallVec<[Prefix; 5]>;
pub type Authorities = SmallVec<[u64; 4]>;

#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub authorities: Authorities,
    pub prefixes: Prefixes,
    pub with_lyrics: Option<bool>,
}

impl GuildConfig {
    pub fn with_lyrics(&self) -> bool {
        self.with_lyrics.unwrap_or(true)
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            authorities: SmallVec::new(),
            prefixes: smallvec!["<".into()],
            with_lyrics: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum OsuData {
    Name(Name),
    User { user_id: u32, username: Name },
}

impl OsuData {
    pub fn username(&self) -> &Name {
        match self {
            Self::Name(username) => username,
            Self::User { username, .. } => username,
        }
    }

    pub fn into_username(self) -> Name {
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

impl From<Name> for OsuData {
    fn from(name: Name) -> Self {
        Self::Name(name)
    }
}

impl From<String> for OsuData {
    fn from(name: String) -> Self {
        Self::Name(name.into())
    }
}

#[derive(Clone, Debug)]
pub struct UserConfig {
    pub embeds_maximized: Option<bool>,
    pub mode: Option<GameMode>,
    pub osu: Option<OsuData>,
    pub profile_size: Option<ProfileSize>,
    pub show_retries: Option<bool>,
    pub twitch_id: Option<u64>,
}

impl UserConfig {
    pub fn username(&self) -> Option<&Name> {
        self.osu.as_ref().map(OsuData::username)
    }

    pub fn into_username(self) -> Option<Name> {
        self.osu.map(OsuData::into_username)
    }

    pub fn embeds_maximized(&self) -> bool {
        self.embeds_maximized.unwrap_or(true)
    }

    pub fn show_retries(&self) -> bool {
        self.show_retries.unwrap_or(true)
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            embeds_maximized: None,
            mode: None,
            osu: None,
            profile_size: None,
            show_retries: None,
            twitch_id: None,
        }
    }
}
