use bathbot_psql::{model::configs::GuildConfig, Database};
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use papaya::HashMap as PapayaMap;
use twilight_model::id::{marker::GuildMarker, Id};

type GuildConfigs = PapayaMap<Id<GuildMarker>, GuildConfig, IntHasher>;

#[derive(Copy, Clone)]
pub struct GuildConfigManager {
    psql: &'static Database,
    guild_configs: &'static GuildConfigs,
}

impl GuildConfigManager {
    pub fn new(psql: &'static Database, guild_configs: &'static GuildConfigs) -> Self {
        Self {
            psql,
            guild_configs,
        }
    }

    /// Get a reference to the [`GuildConfig`] for a guild and execute a
    /// function on it.
    pub async fn peek<F, O>(self, guild_id: Id<GuildMarker>, f: F) -> O
    where
        F: FnOnce(&GuildConfig) -> O,
    {
        if let Some(config) = self.guild_configs.pin().get(&guild_id) {
            return f(config);
        }

        let config = GuildConfig::default();
        let res = f(&config);

        if let Err(err) = self.store(guild_id, config).await {
            warn!("{err:?}");
        }

        res
    }

    pub async fn first_prefix(self, guild_id: Option<Id<GuildMarker>>) -> String {
        let prefix_opt = match guild_id {
            Some(guild_id) => {
                self.peek(guild_id, |config| config.prefixes.first().cloned())
                    .await
            }
            None => None,
        };

        prefix_opt.unwrap_or_else(|| GuildConfig::DEFAULT_PREFIX.to_owned())
    }

    pub async fn update<F, O>(self, guild_id: Id<GuildMarker>, f: F) -> Result<O>
    where
        F: FnOnce(&mut GuildConfig) -> O,
    {
        let mut config = match self.guild_configs.pin().get(&guild_id) {
            Some(config) => config.to_owned(),
            None => GuildConfig::default(),
        };

        let res = f(&mut config);
        self.store(guild_id, config).await?;

        Ok(res)
    }

    async fn store(&self, guild_id: Id<GuildMarker>, config: GuildConfig) -> Result<()> {
        let res = self
            .psql
            .upsert_guild_config(guild_id, &config)
            .await
            .wrap_err("failed to store guild config");

        self.guild_configs.pin().insert(guild_id, config);

        res
    }
}
