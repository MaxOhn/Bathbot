use std::{env, mem::MaybeUninit, path::PathBuf};

use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use rosu_v2::model::Grade;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, UserMarker},
    Id,
};

use crate::{util::Emote, BotResult, Error};

static CONFIG: OnceCell<BotConfig> = OnceCell::new();

#[derive(Debug)]
pub struct BotConfig {
    pub database_url: String,
    pub tokens: Tokens,
    pub paths: Paths,
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
    pub maps: PathBuf,
    pub website: PathBuf,
}

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
    pub osu_daily: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
}

impl BotConfig {
    pub fn get() -> &'static Self {
        CONFIG.get().expect("`BotConfig::init` must be called first")
    }

    pub fn init() -> BotResult<()> {
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
            .collect::<BotResult<_>>()?;

        let config = BotConfig {
            database_url: env_var("DATABASE_URL")?,
            tokens: Tokens {
                discord: env_var("DISCORD_TOKEN")?,
                osu_client_id: env_var("OSU_CLIENT_ID")?,
                osu_client_secret: env_var("OSU_CLIENT_SECRET")?,
                osu_session: env_var("OSU_SESSION")?,
                osu_daily: env_var("OSU_DAILY_TOKEN")?,
                twitch_client_id: env_var("TWITCH_CLIENT_ID")?,
                twitch_token: env_var("TWITCH_TOKEN")?,
            },
            paths: Paths {
                backgrounds: env_var("BG_PATH")?,
                maps: env_var("MAP_PATH")?,
                website: env_var("WEBSITE_PATH")?,
            },
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

    fn from_str(s: &str) -> Option<Self>;
}

macro_rules! env_kind {
    ($($ty:ty: $arg:ident => $impl:block,)*) => {
        $(
            impl EnvKind for $ty {
                const EXPECTED: &'static str = stringify!($ty);

                fn from_str($arg: &str) -> Option<Self> {
                    $impl
                }
            }
        )*
    };
}

env_kind! {
    u16: s => { s.parse().ok() },
    u64: s => { s.parse().ok() },
    PathBuf: s => { s.parse().ok() },
    String: s => { Some(s.to_owned()) },
    Id<UserMarker>: s => { s.parse().ok().map(Id::new) },
    Id<GuildMarker>: s => { s.parse().ok().map(Id::new) },
    Id<ChannelMarker>: s => { s.parse().ok().map(Id::new) },
    [u8; 4]: s => {
        if !(s.starts_with('[') && s.ends_with(']')) {
            return None
        }

        let mut values = s[1..s.len() - 1].split(',');

        let array = [
            values.next()?.trim().parse().ok()?,
            values.next()?.trim().parse().ok()?,
            values.next()?.trim().parse().ok()?,
            values.next()?.trim().parse().ok()?,
        ];

        Some(array)
    },
}

fn env_var<T: EnvKind>(name: &'static str) -> BotResult<T> {
    let value = env::var(name).map_err(|_| Error::MissingEnvVariable(name))?;

    T::from_str(&value).ok_or(Error::ParsingEnvVariable {
        name,
        value,
        expected: T::EXPECTED,
    })
}
