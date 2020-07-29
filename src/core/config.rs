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

// TODO: Use this for emojis
pub static EMOJI_OVERRIDES: OnceCell<HashMap<String, String>> = OnceCell::new();

impl BotConfig {
    pub async fn new(filename: &str) -> BotResult<Self> {
        let config_file = fs::read_to_string(filename)
            .await
            .map_err(|_| Error::NoConfig)?;
        toml::from_str::<BotConfig>(&config_file)
            .map(|c| {
                let mut override_map: HashMap<String, String> = HashMap::new();
                let mut id_map: HashMap<String, u64> = HashMap::new();
                for (name, value) in c.emoji.iter() {
                    override_map.insert(name.clone(), value.clone());
                    let id: u64 = matcher::get_emoji_parts(value)[0].id;
                    id_map.insert(name.clone(), id);
                }
                EMOJI_OVERRIDES.set(override_map).unwrap();
                Ok(c)
            })
            .map_err(Error::InvalidConfig)?
    }
}
