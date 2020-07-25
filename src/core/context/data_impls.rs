use crate::{core::stored_values::Values, database::GuildConfig, BotResult, Context};

use dashmap::DashMap;
use rosu::models::GameMode;
use std::{sync::Arc, time::Instant};
use twilight::model::id::GuildId;

impl Context {
    pub fn get_link(&self, discord_id: u64) -> Option<String> {
        self.data
            .discord_links
            .get(&discord_id)
            .map(|guard| guard.value().to_owned())
    }

    pub async fn add_link(&self, discord_id: u64, osu_name: String) -> BotResult<()> {
        self.clients
            .psql
            .add_discord_link(discord_id, &osu_name)
            .await?;
        self.data.discord_links.insert(discord_id, osu_name);
        Ok(())
    }

    pub async fn remove_link(&self, discord_id: u64) -> BotResult<()> {
        self.clients.psql.remove_discord_link(discord_id).await?;
        self.data.discord_links.remove(&discord_id);
        Ok(())
    }

    pub fn config_authorities(&self, guild_id: GuildId) -> Vec<u64> {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.authorities.clone()
    }

    pub fn config_prefixes(&self, guild_id: GuildId) -> Vec<String> {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.prefixes.clone()
    }

    pub fn config_first_prefix(&self, guild_id: GuildId) -> String {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.prefixes[0].clone()
    }

    pub fn config_lyrics(&self, guild_id: GuildId) -> bool {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.with_lyrics
    }

    pub fn update_config<F>(&self, guild_id: GuildId, f: F)
    where
        F: FnOnce(&mut GuildConfig),
    {
        let mut config = self.data.guilds.entry(guild_id).or_default();
        f(config.value_mut());
    }

    // TODO: Remove
    pub fn guilds(&self) -> &DashMap<GuildId, GuildConfig> {
        &self.data.guilds
    }

    pub fn pp(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.data.stored_values.mania_pp,
            GameMode::CTB => &self.data.stored_values.ctb_pp,
            _ => unreachable!(),
        }
    }

    pub fn stars(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.data.stored_values.mania_stars,
            GameMode::CTB => &self.data.stored_values.ctb_stars,
            _ => unreachable!(),
        }
    }

    /// Intended to use before shutdown
    pub async fn store_configs(&self) -> BotResult<()> {
        let start = Instant::now();
        let guilds = &self.data.guilds;
        let count = self.clients.psql.insert_guilds(guilds).await?;
        let end = Instant::now();
        info!(
            "Stored {} guild configs in {}ms",
            count,
            (end - start).as_millis()
        );
        Ok(())
    }

    /// Intended to use before shutdown
    pub async fn store_values(&self) -> BotResult<()> {
        let start = Instant::now();
        let mania_pp = &self.data.stored_values.mania_pp;
        let mania_stars = &self.data.stored_values.mania_stars;
        let ctb_pp = &self.data.stored_values.ctb_pp;
        let ctb_stars = &self.data.stored_values.ctb_stars;
        let psql = &self.clients.psql;
        let (mania_pp, mania_stars, ctb_pp, ctb_stars) = tokio::try_join!(
            psql.insert_mania_pp(mania_pp),
            psql.insert_mania_stars(mania_stars),
            psql.insert_ctb_pp(ctb_pp),
            psql.insert_ctb_stars(ctb_stars),
        )?;
        let end = Instant::now();
        info!(
            "Stored {} pp and {} star values in {}ms",
            mania_pp + ctb_pp,
            mania_stars + ctb_stars,
            (end - start).as_millis()
        );
        Ok(())
    }
}
