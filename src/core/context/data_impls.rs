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

    pub async fn get_config(guild_id: &GuildId) -> Option<Arc<GuildConfig>> {
        todo!()
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
