use crate::{util::matcher, BotResult, Error};

use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use tokio::fs;

#[derive(Deserialize, Debug)]
pub struct BotConfig {
    pub tokens: Tokens,
    pub database: Database,
    pub bg_path: PathBuf,
    pub emoji: HashMap<String, String>,
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
    pub async fn new(filename: &str) -> BotResult<Self> {
        let config_file = fs::read_to_string(filename)
            .await
            .map_err(|_| Error::NoConfig)?;
        toml::from_str::<BotConfig>(&config_file).map_err(Error::InvalidConfig)
    }
}
