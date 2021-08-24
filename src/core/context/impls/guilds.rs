use crate::{
    database::{Authorities, GuildConfig, Prefix, Prefixes},
    BotResult, Context,
};

use dashmap::mapref::entry::Entry;
use twilight_model::id::GuildId;

impl Context {
    pub async fn config_authorities(&self, guild_id: GuildId) -> Authorities {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().insert_guild(guild_id, &config).await {
                    unwind_error!(
                        warn,
                        why,
                        "Failed to insert guild {} in config_authorities: {}",
                        guild_id
                    );
                }

                entry.insert(config)
            }
        };

        config.authorities.clone()
    }

    pub async fn config_authorities_collect<F, T>(&self, guild_id: GuildId, f: F) -> Vec<T>
    where
        F: FnMut(u64) -> T,
    {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().insert_guild(guild_id, &config).await {
                    unwind_error!(
                        warn,
                        why,
                        "Failed to insert guild {} in config_authorities_collect: {}",
                        guild_id
                    );
                }

                entry.insert(config)
            }
        };

        config.authorities.iter().copied().map(f).collect()
    }

    pub async fn config_prefixes(&self, guild_id: GuildId) -> Prefixes {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().insert_guild(guild_id, &config).await {
                    unwind_error!(
                        warn,
                        why,
                        "Failed to insert guild {} in config_prefixes: {}",
                        guild_id
                    );
                }

                entry.insert(config)
            }
        };

        config.prefixes.clone()
    }

    pub async fn config_first_prefix(&self, guild_id: Option<GuildId>) -> Prefix {
        match guild_id {
            Some(guild_id) => {
                let config = match self.data.guilds.entry(guild_id) {
                    Entry::Occupied(entry) => entry.into_ref(),
                    Entry::Vacant(entry) => {
                        let config = GuildConfig::default();

                        if let Err(why) = self.psql().insert_guild(guild_id, &config).await {
                            unwind_error!(
                                warn,
                                why,
                                "Failed to insert guild {} in config_first_prefix: {}",
                                guild_id
                            );
                        }

                        entry.insert(config)
                    }
                };

                config.prefixes[0].clone()
            }
            None => "<".into(),
        }
    }

    pub async fn config_lyrics(&self, guild_id: GuildId) -> bool {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().insert_guild(guild_id, &config).await {
                    unwind_error!(
                        warn,
                        why,
                        "Failed to insert guild {} in config_lyrics: {}",
                        guild_id
                    );
                }

                entry.insert(config)
            }
        };

        config.with_lyrics
    }

    pub async fn update_config<F>(&self, guild_id: GuildId, f: F) -> BotResult<()>
    where
        F: FnOnce(&mut GuildConfig),
    {
        let mut config = self.data.guilds.entry(guild_id).or_default();
        f(config.value_mut());

        self.psql().insert_guild(guild_id, &config).await
    }
}
