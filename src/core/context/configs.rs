use dashmap::mapref::{entry::Entry, one::RefMut};
use eyre::Report;
use twilight_model::id::{
    marker::{GuildMarker, UserMarker},
    Id,
};

use crate::{
    commands::osu::ProfileSize,
    core::commands::prefix::Stream,
    database::{Authorities, EmbedsSize, GuildConfig, MinimizedPp, Prefix, Prefixes, UserConfig},
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

    async fn guild_config_ref(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> RefMut<'_, Id<GuildMarker>, GuildConfig> {
        match self.data.guilds.entry(guild_id) {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => {
                let config = GuildConfig::default();

                if let Err(err) = self.psql().upsert_guild_config(guild_id, &config).await {
                    let wrap = format!("failed to insert guild {guild_id}");
                    let report = Report::new(err).wrap_err(wrap);
                    warn!("{report:?}");
                }

                entry.insert(config)
            }
        }
    }

    pub async fn guild_authorities(&self, guild_id: Id<GuildMarker>) -> Authorities {
        self.guild_config_ref(guild_id).await.authorities.clone()
    }

    pub async fn guild_prefixes(&self, guild_id: Id<GuildMarker>) -> Prefixes {
        self.guild_config_ref(guild_id).await.prefixes.clone()
    }

    pub async fn guild_prefixes_find(
        &self,
        guild_id: Id<GuildMarker>,
        stream: &Stream<'_>,
    ) -> Option<Prefix> {
        self.guild_config_ref(guild_id)
            .await
            .prefixes
            .iter()
            .find(|p| stream.starts_with(p))
            .cloned()
    }

    pub async fn guild_first_prefix(&self, guild_id: Option<Id<GuildMarker>>) -> Prefix {
        match guild_id {
            Some(guild_id) => self.guild_config_ref(guild_id).await.prefixes[0].clone(),
            None => "<".into(),
        }
    }

    pub async fn guild_with_lyrics(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_config_ref(guild_id).await.with_lyrics()
    }

    pub async fn guild_profile_size(&self, guild_id: Id<GuildMarker>) -> ProfileSize {
        self.guild_config_ref(guild_id).await.profile_size()
    }

    pub async fn guild_show_retries(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_config_ref(guild_id).await.show_retries()
    }

    pub async fn guild_embeds_maximized(&self, guild_id: Id<GuildMarker>) -> EmbedsSize {
        self.guild_config_ref(guild_id).await.embeds_size()
    }

    pub async fn guild_track_limit(&self, guild_id: Id<GuildMarker>) -> u8 {
        self.guild_config_ref(guild_id).await.track_limit()
    }

    pub async fn guild_minimized_pp(&self, guild_id: Id<GuildMarker>) -> MinimizedPp {
        self.guild_config_ref(guild_id).await.minimized_pp()
    }

    pub async fn guild_config(&self, guild_id: Id<GuildMarker>) -> GuildConfig {
        self.guild_config_ref(guild_id).await.to_owned()
    }

    pub async fn update_guild_config<F>(&self, guild_id: Id<GuildMarker>, f: F) -> BotResult<()>
    where
        F: FnOnce(&mut GuildConfig),
    {
        let mut config = self.data.guilds.entry(guild_id).or_default();
        f(config.value_mut());

        self.psql().upsert_guild_config(guild_id, &config).await
    }
}
