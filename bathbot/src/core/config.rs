use std::{env, fmt::Debug, mem::MaybeUninit, path::PathBuf, str::FromStr};

use eyre::Result;
use once_cell::sync::OnceCell;
use rosu_v2::model::Grade;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, UserMarker},
    Id,
};

use crate::util::{CustomEmote, Emote};

static CONFIG: OnceCell<BotConfig> = OnceCell::new();

#[derive(Debug)]
pub struct BotConfig {
    pub database_url: Box<str>,
    pub tokens: Tokens,
    pub paths: Paths,
    #[cfg(feature = "server")]
    pub server: Server,
    grades: Box<[Box<str>; 9]>, // TODO: remove length
    emotes: Box<[CustomEmote; 16]>,
    pub redis_host: Box<str>,
    pub redis_port: u16,
    pub redis_db_idx: u8,
    pub owner: Id<UserMarker>,
    pub dev_guild: Id<GuildMarker>,
    pub hl_channel: Id<ChannelMarker>,
}

#[derive(Debug)]
pub struct Paths {
    pub backgrounds: PathBuf,
    pub assets: PathBuf,
    pub maps: PathBuf,
    #[cfg(feature = "server")]
    pub website: PathBuf,
}

#[cfg(feature = "server")]
#[derive(Debug)]
pub struct Server {
    pub port: u16,
    pub public_url: Box<str>,
}

#[derive(Debug)]
pub struct Tokens {
    pub discord: Box<str>,
    pub osu_client_id: u64,
    pub osu_client_secret: Box<str>,
    pub osu_key: Box<str>,
    #[cfg(not(debug_assertions))]
    pub ordr_key: Box<str>,
    pub github_token: Box<str>,
    #[cfg(feature = "twitch")]
    pub twitch_client_id: Box<str>,
    #[cfg(feature = "twitch")]
    pub twitch_token: Box<str>,
}

impl BotConfig {
    pub fn get() -> &'static Self {
        CONFIG
            .get()
            .expect("`BotConfig::init` must be called first")
    }

    pub fn init() -> Result<()> {
        let grade_strs = ["F", "D", "C", "B", "A", "S", "X", "SH", "XH"];
        let grades = Self::parse_emotes::<Grade, _, 9>(grade_strs)?;

        let emote_strs = [
            "osu",
            "osu_std",
            "osu_taiko",
            "osu_ctb",
            "osu_mania",
            "twitch",
            "tracking",
            "jump_start",
            "single_step_back",
            "my_position",
            "single_step",
            "jump_end",
            "miss",
            "bpm",
            "count_objects",
            "count_spinners",
        ];
        let emotes = Self::parse_emotes::<Emote, _, 16>(emote_strs)?;

        let config = BotConfig {
            database_url: env_var("DATABASE_URL")?,
            tokens: Tokens {
                discord: env_var("DISCORD_TOKEN")?,
                osu_client_id: env_var("OSU_CLIENT_ID")?,
                osu_client_secret: env_var("OSU_CLIENT_SECRET")?,
                osu_key: env_var("OSU_API_KEY")?,
                #[cfg(not(debug_assertions))]
                ordr_key: env_var("ORDR_KEY")?,
                github_token: env_var("GITHUB_TOKEN")?,
                #[cfg(feature = "twitch")]
                twitch_client_id: env_var("TWITCH_CLIENT_ID")?,
                #[cfg(feature = "twitch")]
                twitch_token: env_var("TWITCH_TOKEN")?,
            },
            paths: Paths {
                backgrounds: env_var("BG_PATH")?,
                assets: env_var("ASSETS_PATH")?,
                maps: env_var("MAP_PATH")?,
                #[cfg(feature = "server")]
                website: env_var("WEBSITE_PATH")?,
            },
            #[cfg(feature = "server")]
            server: Server {
                port: env_var("SERVER_PORT")?,
                public_url: env_var("PUBLIC_URL")?,
            },
            grades,
            emotes,
            redis_host: env_var("REDIS_HOST")?,
            redis_port: env_var("REDIS_PORT")?,
            redis_db_idx: env_var("REDIS_DB_IDX")?,
            owner: env_var("OWNER_USER_ID")?,
            dev_guild: env_var("DEV_GUILD_ID")?,
            hl_channel: env_var("HL_IMAGE_CHANNEL")?,
        };

        if CONFIG.set(config).is_err() {
            warn!("CONFIG was already set");
        }

        Ok(())
    }

    fn parse_emotes<K, V, const N: usize>(names: [&str; N]) -> Result<Box<[V; N]>>
    where
        K: FromStr + AsUsize,
        V: EnvKind,
    {
        let mut emotes = Box::new([(); N].map(|_| MaybeUninit::uninit()));

        for name in names {
            let Ok(key) = name.parse::<K>() else {
                unreachable!()
            };
            let value: V = env_var(name)?;
            emotes[key.to_usize()].write(value);
        }

        // SAFETY: All emotes have been initialized.
        // Otherwise an error would have been thrown due to a missing emote.
        Ok(unsafe { Box::from_raw(Box::into_raw(emotes) as *mut [V; N]) })
    }

    pub fn grade(&self, grade: Grade) -> &str {
        self.grades[grade as usize].as_ref()
    }

    pub fn emote(&self, emote: Emote) -> &CustomEmote {
        &self.emotes[emote as usize]
    }
}

trait EnvKind: Sized {
    const EXPECTED: &'static str;

    fn from_str(s: String) -> Result<Self, String>;
}

macro_rules! env_kind {
    ($($ty:ty: |$arg:ident| $impl:block,)*) => {
        $(
            impl EnvKind for $ty {
                const EXPECTED: &'static str = stringify!($ty);

                fn from_str($arg: String) -> Result<Self, String> {
                    $impl
                }
            }
        )*
    };
}

env_kind! {
    Box<str>: |s| { Ok(s.into_boxed_str()) },
    u8: |s| { s.parse().map_err(|_| s) },
    u16: |s| { s.parse().map_err(|_| s) },
    u64: |s| { s.parse().map_err(|_| s) },
    PathBuf: |s| { s.parse().map_err(|_| s) },
    Id<UserMarker>: |s| { s.parse().map(Id::new).map_err(|_| s) },
    Id<GuildMarker>: |s| { s.parse().map(Id::new).map_err(|_| s) },
    Id<ChannelMarker>: |s| { s.parse().map(Id::new).map_err(|_| s) },
}

impl EnvKind for CustomEmote {
    const EXPECTED: &'static str = "an emote of the form `<:name:id>`";

    fn from_str(s: String) -> Result<Self, String> {
        fn inner(s: &str) -> Option<CustomEmote> {
            let (name, id) = s.strip_prefix("<:")?.strip_suffix('>')?.split_once(':')?;
            let id = id.parse().ok()?;

            Some(CustomEmote::new(id, Box::from(name)))
        }

        inner(s.as_str()).ok_or(s)
    }
}

fn env_var<T: EnvKind>(name: &str) -> Result<T> {
    let value = env::var(name).map_err(|_| eyre!("missing env variable `{name}`"))?;

    T::from_str(value).map_err(|value| {
        eyre!(
            "failed to parse env variable `{name}={value}`; expected {expected}",
            expected = T::EXPECTED
        )
    })
}

trait AsUsize {
    fn to_usize(self) -> usize;
}

impl AsUsize for Grade {
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl AsUsize for Emote {
    fn to_usize(self) -> usize {
        self as usize
    }
}
