use dashmap::mapref::entry::Entry;
use eyre::Report;
use twilight_model::id::{
    marker::{GuildMarker, UserMarker},
    Id,
};

use crate::{
    commands::osu::ProfileSize,
    database::{Authorities, GuildConfig, Prefix, Prefixes, UserConfig},
    BotResult, Context,
};

impl Context {
    pub async fn user_config(&self, user_id: Id<UserMarker>) -> BotResult<UserConfig> {
        match self.psql().get_user_config(user_id).await? {
            Some(config) => Ok(config),
            None => {
                let config = UserConfig::default();
                self.psql().insert_user_config(user_id, &config).await?;

                Ok(config)
            }
        }
    }

    pub async fn guild_authorities(&self, guild_id: Id<GuildMarker>) -> Authorities {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }

                entry.insert(config)
            }
        };

        config.authorities.clone()
    }

    pub async fn guild_prefixes(&self, guild_id: Id<GuildMarker>) -> Prefixes {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }

                entry.insert(config)
            }
        };

        config.prefixes.clone()
    }

    pub async fn guild_first_prefix(&self, guild_id: Option<Id<GuildMarker>>) -> Prefix {
        match guild_id {
            Some(guild_id) => {
                let config = match self.data.guilds.entry(guild_id) {
                    Entry::Occupied(entry) => entry.into_ref(),
                    Entry::Vacant(entry) => {
                        let config = GuildConfig::default();

                        if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                            let wrap = format!("failed to insert guild {guild_id}");
                            let report = Report::new(why).wrap_err(wrap);
                            warn!("{:?}", report);
                        }

                        entry.insert(config)
                    }
                };

                config.prefixes[0].clone()
            }
            None => "<".into(),
        }
    }

    pub async fn guild_with_lyrics(&self, guild_id: Id<GuildMarker>) -> bool {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }

                entry.insert(config)
            }
        };

        config.with_lyrics()
    }

    pub async fn guild_profile_size(&self, guild_id: Id<GuildMarker>) -> ProfileSize {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }

                entry.insert(config)
            }
        };

        config.profile_size.unwrap_or_default()
    }

    pub async fn guild_show_retries(&self, guild_id: Id<GuildMarker>) -> bool {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                }

                entry.insert(config)
            }
        };

        config.show_retries()
    }

    pub async fn guild_embeds_maximized(&self, guild_id: Id<GuildMarker>) -> bool {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{report:?}");
                }

                entry.insert(config)
            }
        };

        config.embeds_maximized()
    }

    // TODO: Refactor all these methods
    pub async fn guild_track_limit(&self, guild_id: Id<GuildMarker>) -> u8 {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{report:?}");
                }

                entry.insert(config)
            }
        };

        config.track_limit()
    }

    pub async fn update_guild_config<F>(&self, guild_id: Id<GuildMarker>, f: F) -> BotResult<()>
    where
        F: FnOnce(&mut GuildConfig),
    {
        let mut config = self.data.guilds.entry(guild_id).or_default();
        f(config.value_mut());

        self.psql().upsert_guild_config(guild_id, &config).await
    }

    pub async fn guild_config(&self, guild_id: Id<GuildMarker>) -> GuildConfig {
        let config = match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(why) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{report:?}");
                }

                entry.insert(config)
            }
        };

        config.to_owned()
    }
}
