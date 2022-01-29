use crate::{util::Emote, BotResult, Error};

use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use rosu_v2::model::Grade;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::fs;

pub static CONFIG: OnceCell<BotConfig> = OnceCell::new();

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub tokens: Tokens,
    pub bg_path: PathBuf,
    pub map_path: PathBuf,
    pub server: Server,
    grades: HashMap<Grade, String>,
    pub emotes: HashMap<Emote, String>,
    pub redis_host: String,
    pub redis_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub internal_ip: [u8; 4],
    pub internal_port: u16,
    pub external_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Tokens {
    pub discord: String,
    pub osu_client_id: u64,
    pub osu_client_secret: String,
    pub osu_session: String,
    pub osu_token: String,
    pub osu_daily: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
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
}
