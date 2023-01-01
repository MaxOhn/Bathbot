use std::{env, mem::MaybeUninit, path::PathBuf};

use eyre::Result;
use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use rosu_v2::model::Grade;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, UserMarker},
    Id,
};

use crate::util::Emote;

static CONFIG: OnceCell<BotConfig> = OnceCell::new();

#[derive(Debug)]
pub struct BotConfig {
    pub database_url: String,
    pub tokens: Tokens,
    pub paths: Paths,
    #[cfg(feature = "server")]
    pub server: Server,
    grades: [String; 9],
    pub emotes: HashMap<Emote, String>,
    pub redis_host: String,
    pub redis_port: u16,
    pub owner: Id<UserMarker>,
    pub dev_guild: Id<GuildMarker>,
    pub hl_channel: Id<ChannelMarker>,
}

#[derive(Debug)]
pub struct Paths {
    pub backgrounds: PathBuf,
    pub cards: PathBuf,
    pub maps: PathBuf,
    #[cfg(feature = "server")]
    pub website: PathBuf,
}

#[cfg(feature = "server")]
#[derive(Debug)]
pub struct Server {
    pub internal_ip: [u8; 4],
    pub internal_port: u16,
    pub external_url: String,
}

#[derive(Debug)]
pub struct Tokens {
    pub discord: String,
    pub osu_client_id: u64,
    pub osu_client_secret: String,
    pub osu_session: String,
    #[cfg(feature = "twitch")]
    pub twitch_client_id: String,
    #[cfg(feature = "twitch")]
    pub twitch_token: String,
}

impl BotConfig {
    pub fn get() -> &'static Self {
        CONFIG
            .get()
            .expect("`BotConfig::init` must be called first")
    }

    pub fn init() -> Result<()> {
        let mut grades = [
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        ];

        let grade_strs = ["F", "D", "C", "B", "A", "S", "X", "SH", "XH"];

        for grade_str in grade_strs {
            let key: Grade = grade_str.parse().unwrap();
            let value: String = env_var(grade_str)?;
            grades[key as usize].write(value);
        }

        // SAFETY: All grades have been initialized.
        // Otherwise an error would have been thrown due to a missing emote.
        let grades = unsafe { (&grades as *const _ as *const [String; 9]).read() };

        let emotes = [
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
        ];

        let emotes = emotes
            .iter()
            .map(|emote_str| {
                let key = emote_str.parse().unwrap();
                let value = env_var(emote_str)?;

                Ok((key, value))
            })
            .collect::<Result<_>>()?;

        let config = BotConfig {
            database_url: env_var("DATABASE_URL")?,
            tokens: Tokens {
                discord: env_var("DISCORD_TOKEN")?,
                osu_client_id: env_var("OSU_CLIENT_ID")?,
                osu_client_secret: env_var("OSU_CLIENT_SECRET")?,
                osu_session: env_var("OSU_SESSION")?,
                #[cfg(feature = "twitch")]
                twitch_client_id: env_var("TWITCH_CLIENT_ID")?,
                #[cfg(feature = "twitch")]
                twitch_token: env_var("TWITCH_TOKEN")?,
            },
            paths: Paths {
                backgrounds: env_var("BG_PATH")?,
                cards: env_var("CARDS_REPO_PATH")?,
                maps: env_var("MAP_PATH")?,
                #[cfg(feature = "server")]
                website: env_var("WEBSITE_PATH")?,
            },
            #[cfg(feature = "server")]
            server: Server {
                internal_ip: env_var("INTERNAL_IP")?,
                internal_port: env_var("INTERNAL_PORT")?,
                external_url: env_var("EXTERNAL_URL")?,
            },
            grades,
            emotes,
            redis_host: env_var("REDIS_HOST")?,
            redis_port: env_var("REDIS_PORT")?,
            owner: env_var("OWNER_USER_ID")?,
            dev_guild: env_var("DEV_GUILD_ID")?,
            hl_channel: env_var("HL_IMAGE_CHANNEL")?,
        };

        if CONFIG.set(config).is_err() {
            warn!("CONFIG was already set");
        }

        Ok(())
    }

    pub fn grade(&self, grade: Grade) -> &str {
        self.grades[grade as usize].as_str()
    }
}

trait EnvKind: Sized {
    const EXPECTED: &'static str;

    fn from_str(s: String) -> Result<Self, String>;
}

macro_rules! env_kind {
    ($($ty:ty: $arg:ident => $impl:block,)*) => {
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
    String: s => { Ok(s) },
    u16: s => { s.parse().map_err(|_| s) },
    u64: s => { s.parse().map_err(|_| s) },
    PathBuf: s => { s.parse().map_err(|_| s) },
    Id<UserMarker>: s => { s.parse().map(Id::new).map_err(|_| s) },
    Id<GuildMarker>: s => { s.parse().map(Id::new).map_err(|_| s) },
    Id<ChannelMarker>: s => { s.parse().map(Id::new).map_err(|_| s) },
    [u8; 4]: s => {
        if !(s.starts_with('[') && s.ends_with(']')) {
            return Err(s);
        }

        let mut values = s[1..s.len() - 1]
            .split(',')
            .map(str::trim)
            .flat_map(str::parse);

        let mut array = [0; 4];

        for idx in array.iter_mut() {
            *idx = if let Some(n) = values.next() {
                n
            } else {
                return Err(s);
            };
        }

        Ok(array)
    },
}

fn env_var<T: EnvKind>(name: &'static str) -> Result<T> {
    let value = env::var(name).map_err(|_| eyre!("missing env variable `{name}`"))?;

    T::from_str(value).map_err(|value| {
        eyre!(
            "failed to parse env variable `{name}={value}`; expected {expected}",
            expected = T::EXPECTED
        )
    })
}
