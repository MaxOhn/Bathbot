use crate::{BotResult, Error};

use once_cell::sync::OnceCell;
use rosu::models::Grade;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
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
    pub emotes: HashMap<Grade, String>,
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
        self.emotes
            .get(&grade)
            .unwrap_or_else(|| panic!("No grade emote for grade {} in config", grade))
    }
}

pub static CONFIG: OnceCell<BotConfig> = OnceCell::new();
