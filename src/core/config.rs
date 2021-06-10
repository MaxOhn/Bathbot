use crate::{BotResult, Error};

use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use rosu_v2::model::Grade;
use serde::{
    de::{Deserializer, Error as SerdeError, Unexpected},
    Deserialize,
};
use std::{path::PathBuf, str::FromStr};
use tokio::fs;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::id::EmojiId;

#[derive(Deserialize, Debug)]
pub struct BotConfig {
    pub tokens: Tokens,
    pub bg_path: PathBuf,
    pub map_path: PathBuf,
    pub metric_server_ip: [u8; 4],
    pub metric_server_port: u16,
    grades: HashMap<Grade, String>,
    emotes: HashMap<Emote, String>,
}

#[derive(Deserialize, Debug)]
pub struct Tokens {
    pub discord: String,
    pub osu_client_id: u64,
    pub osu_client_secret: String,
    pub osu_session: String,
    pub osu_daily: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Emote {
    Std,
    Tko,
    Ctb,
    Mna,

    Minimize,
    Expand,

    JumpStart,
    MultiStepBack,
    SingleStepBack,
    MyPosition,
    SingleStep,
    MultiStep,
    JumpEnd,

    Custom(&'static str),
}

impl Emote {
    #[inline]
    pub fn text(self) -> &'static str {
        CONFIG.get().unwrap().emotes.get(&self).unwrap()
    }

    pub fn request_reaction(&self) -> RequestReactionType {
        let emotes = &CONFIG.get().unwrap().emotes;

        let emote = if let Self::Custom(emote) = self {
            return RequestReactionType::Unicode {
                name: emote.to_string(),
            };
        } else {
            emotes.get(self)
        };

        let (id, name) = emote
            .unwrap_or_else(|| panic!("No {:?} emote in config", self))
            .split_emote();

        RequestReactionType::Custom {
            id: EmojiId(id),
            name: Some(name.to_owned()),
        }
    }
}

impl BotConfig {
    pub async fn init(filename: &str) -> BotResult<()> {
        let config_file = fs::read_to_string(filename)
            .await
            .map_err(|_| Error::NoConfig)?;

        let config = toml::from_str::<BotConfig>(&config_file).map_err(Error::InvalidConfig)?;

        if CONFIG.set(config).is_err() {
            warn!("CONFIG was already set");
        }

        Ok(())
    }

    #[inline]
    pub fn grade(&self, grade: Grade) -> &str {
        self.grades
            .get(&grade)
            .unwrap_or_else(|| panic!("No grade emote for grade {} in config", grade))
    }
}

pub static CONFIG: OnceCell<BotConfig> = OnceCell::new();

trait SplitEmote {
    fn split_emote(&self) -> (u64, &str);
}

impl SplitEmote for String {
    #[inline]
    fn split_emote(&self) -> (u64, &str) {
        let mut split = self.split(':');
        let name = split.nth(1).unwrap();
        let id = split.next().unwrap();
        let id = u64::from_str(&id[0..id.len() - 1]).unwrap();

        (id, name)
    }
}

impl<'de> Deserialize<'de> for Emote {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        let other = match s {
            "osu_std" => Self::Std,
            "osu_taiko" => Self::Tko,
            "osu_ctb" => Self::Ctb,
            "osu_mania" => Self::Mna,
            "minimize" => Self::Minimize,
            "expand" => Self::Expand,
            "jump_start" => Self::JumpStart,
            "multi_step_back" => Self::MultiStepBack,
            "single_step_back" => Self::SingleStepBack,
            "my_position" => Self::MyPosition,
            "single_step" => Self::SingleStep,
            "multi_step" => Self::MultiStep,
            "jump_end" => Self::JumpEnd,
            other => {
                return Err(SerdeError::invalid_value(
                    Unexpected::Str(other),
                    &"the name of a required emote",
                ))
            }
        };

        Ok(other)
    }
}
