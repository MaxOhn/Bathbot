use eyre::{Result, WrapErr};
use twilight_model::id::{
    marker::{GuildMarker, UserMarker},
    Id,
};

use crate::{
    commands::osu::ProfileSize,
    core::commands::prefix::Stream,
    database::{
        Authorities, EmbedsSize, GuildConfig, ListSize, MinimizedPp, Prefix, Prefixes, UserConfig,
    },
    Context,
};

impl Context {
    pub async fn user_config(&self, user_id: Id<UserMarker>) -> Result<UserConfig> {
        let config_fut = self.psql().get_user_config(user_id);

        match config_fut.await.wrap_err("failed to get user config")? {
            Some(config) => Ok(config),
            None => {
                let config = UserConfig::default();

                self.psql()
                    .insert_user_config(user_id, &config)
                    .await
                    .wrap_err("failed to insert default user config")?;

                Ok(config)
            }
        }
    }

    async fn guild_config_<'g, F, O>(&self, guild_id: Id<GuildMarker>, f: F) -> O
    where
        F: FnOnce(&GuildConfig) -> O,
    {
        if let Some(config) = self.data.guilds.pin().get(&guild_id) {
            return f(config);
        }

        let config = GuildConfig::default();

        if let Err(err) = self.psql().upsert_guild_config(guild_id, &config).await {
            let wrap = format!("failed to insert guild {guild_id}");
            warn!("{:?}", err.wrap_err(wrap));
        }

        let res = f(&config);
        self.data.guilds.pin().insert(guild_id, config);

        res
    }

    pub async fn guild_authorities(&self, guild_id: Id<GuildMarker>) -> Authorities {
        self.guild_config_(guild_id, |config| config.authorities.clone())
            .await
    }

    pub async fn guild_prefixes(&self, guild_id: Id<GuildMarker>) -> Prefixes {
        self.guild_config_(guild_id, |config| config.prefixes.clone())
            .await
    }

    pub async fn guild_prefixes_find(
        &self,
        guild_id: Id<GuildMarker>,
        stream: &Stream<'_>,
    ) -> Option<Prefix> {
        let f = |config: &GuildConfig| {
            config
                .prefixes
                .iter()
                .find(|p| stream.starts_with(p))
                .cloned()
        };

        self.guild_config_(guild_id, f).await
    }

    pub async fn guild_first_prefix(&self, guild_id: Option<Id<GuildMarker>>) -> Prefix {
        match guild_id {
            Some(guild_id) => {
                self.guild_config_(guild_id, |config| config.prefixes[0].clone())
                    .await
            }
            None => "<".into(),
        }
    }

    pub async fn guild_with_lyrics(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_config_(guild_id, GuildConfig::with_lyrics).await
    }

    pub async fn guild_profile_size(&self, guild_id: Id<GuildMarker>) -> ProfileSize {
        self.guild_config_(guild_id, GuildConfig::profile_size)
            .await
    }

    pub async fn guild_show_retries(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_config_(guild_id, GuildConfig::show_retries)
            .await
    }

    pub async fn guild_embeds_maximized(&self, guild_id: Id<GuildMarker>) -> EmbedsSize {
        self.guild_config_(guild_id, GuildConfig::embeds_size).await
    }

    pub async fn guild_track_limit(&self, guild_id: Id<GuildMarker>) -> u8 {
        self.guild_config_(guild_id, GuildConfig::track_limit).await
    }

    pub async fn guild_minimized_pp(&self, guild_id: Id<GuildMarker>) -> MinimizedPp {
        self.guild_config_(guild_id, GuildConfig::minimized_pp)
            .await
    }

    pub async fn guild_list_size(&self, guild_id: Id<GuildMarker>) -> ListSize {
        self.guild_config_(guild_id, GuildConfig::list_size).await
    }

    pub async fn guild_config(&self, guild_id: Id<GuildMarker>) -> GuildConfig {
        self.guild_config_(guild_id, GuildConfig::to_owned).await
    }

    pub async fn update_guild_config<F>(&self, guild_id: Id<GuildMarker>, f: F) -> Result<()>
    where
        F: FnOnce(&mut GuildConfig),
    {
        let guilds = &self.data.guilds;

        let mut config = guilds
            .pin()
            .get(&guild_id)
            .map(GuildConfig::to_owned)
            .unwrap_or_default();

        f(&mut config);

        self.psql()
            .upsert_guild_config(guild_id, &config)
            .await
            .wrap_err("failed to upsert guild config")?;

        guilds.pin().insert(guild_id, config);

        Ok(())
    }
}
