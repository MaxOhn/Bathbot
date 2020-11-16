use crate::{BotResult, Error};

use once_cell::sync::OnceCell;
use rosu::model::{GameMode, Grade};
use serde::Deserialize;
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
    modes: HashMap<GameMode, String>,
}

#[derive(Deserialize, Debug)]
pub struct Tokens {
    pub discord: String,
    pub osu: String,
    pub osu_session: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
}

#[derive(Deserialize, Debug)]
pub struct Database {
    pub postgres: String,
    pub redis: String,
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
    pub fn mode(&self, mode: GameMode) -> &str {
        self.modes
            .get(&mode)
            .unwrap_or_else(|| panic!("No mode emote for mode {} in config", mode))
    }
    pub fn all_modes(&self) -> [(u64, &str); 4] {
        let std = self.single_mode(GameMode::STD);
        let tko = self.single_mode(GameMode::TKO);
        let ctb = self.single_mode(GameMode::CTB);
        let mna = self.single_mode(GameMode::MNA);
        [std, tko, ctb, mna]
    }
    fn single_mode(&self, mode: GameMode) -> (u64, &str) {
        let mut split = self.modes[&mode].split(':');
        let name = split.nth(1).unwrap();
        let id = split.next().unwrap();
        let id = u64::from_str(&id[0..id.len() - 1]).unwrap();
        (id, name)
    }
}

pub static CONFIG: OnceCell<BotConfig> = OnceCell::new();
