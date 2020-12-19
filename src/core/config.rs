use crate::{BotResult, Error};

use once_cell::sync::OnceCell;
use rosu::model::{GameMode, Grade};
use serde::{
    de::{Deserializer, Error as SerdeError, Unexpected},
    Deserialize,
};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use tokio::fs;

#[derive(Deserialize, Debug)]
pub struct BotConfig {
    pub tokens: Tokens,
    pub database: Database,
    pub bg_path: PathBuf,
    pub map_path: PathBuf,
    pub perf_calc_path: PathBuf,
    pub metric_server_ip: [u8; 4],
    pub metric_server_port: u16,
    grades: HashMap<Grade, String>,
    pub modes: HashMap<GameMode, String>,
    #[serde(rename = "other")]
    other: HashMap<OtherEnum, String>,
}

#[derive(Deserialize, Debug)]
pub struct Tokens {
    pub discord: String,
    pub osu: String,
    pub osu_session: String,
    pub osu_daily: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
}

#[derive(Deserialize, Debug)]
pub struct Database {
    pub host: String,
    pub db_user: String,
    pub db_pw: String,
    pub db_name: String,
    pub redis_port: u16,
}

#[derive(Eq, PartialEq, Debug, Hash)]
pub enum OtherEnum {
    Minimize,
    Expand,
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

    pub fn grade(&self, grade: Grade) -> &str {
        self.grades
            .get(&grade)
            .unwrap_or_else(|| panic!("No grade emote for grade {} in config", grade))
    }

    #[allow(dead_code)]
    pub fn mode(&self, mode: GameMode) -> (u64, &str) {
        self.modes
            .get(&mode)
            .unwrap_or_else(|| panic!("No mode emote for mode {} in config", mode))
            .split_emote()
    }

    #[allow(dead_code)]
    pub fn all_modes(&self) -> [(u64, &str); 4] {
        let std = self.modes[&GameMode::STD].split_emote();
        let tko = self.modes[&GameMode::TKO].split_emote();
        let ctb = self.modes[&GameMode::CTB].split_emote();
        let mna = self.modes[&GameMode::MNA].split_emote();

        [std, tko, ctb, mna]
    }

    pub fn minimize(&self) -> (u64, &str) {
        self.other
            .get(&OtherEnum::Minimize)
            .unwrap_or_else(|| panic!("No minimize emote in config"))
            .split_emote()
    }

    pub fn expand(&self) -> (u64, &str) {
        self.other
            .get(&OtherEnum::Expand)
            .unwrap_or_else(|| panic!("No expand emote in config"))
            .split_emote()
    }
}

pub static CONFIG: OnceCell<BotConfig> = OnceCell::new();

trait SplitEmote {
    fn split_emote(&self) -> (u64, &str);
}

impl SplitEmote for String {
    fn split_emote(&self) -> (u64, &str) {
        let mut split = self.split(':');
        let name = split.nth(1).unwrap();
        let id = split.next().unwrap();
        let id = u64::from_str(&id[0..id.len() - 1]).unwrap();

        (id, name)
    }
}

impl<'de> Deserialize<'de> for OtherEnum {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(d)?;

        match s {
            "minimize" => Ok(Self::Minimize),
            "expand" => Ok(Self::Expand),
            other => Err(SerdeError::invalid_value(
                Unexpected::Str(other),
                &r#""minimize" or "expand""#,
            )),
        }
    }
}
