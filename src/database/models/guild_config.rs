use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct GuildConfig {
    pub with_lyrics: bool,
    pub prefixes: Vec<String>,
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            with_lyrics: true,
            prefixes: vec!["<".to_owned(), "!!".to_owned()],
        }
    }
}
