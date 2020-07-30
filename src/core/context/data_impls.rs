use crate::{
    bg_game::GameWrapper,
    core::stored_values::Values,
    database::{GuildConfig, MapsetTagWrapper},
    util::error::BgGameError,
    BotResult, Context,
};

use rosu::models::GameMode;
use std::{sync::Arc, time::Instant};
use tokio::sync::Mutex;
use twilight::model::{
    channel::Reaction,
    id::{ChannelId, GuildId, MessageId, RoleId},
};

impl Context {
    pub fn get_link(&self, discord_id: u64) -> Option<String> {
        self.data
            .discord_links
            .get(&discord_id)
            .map(|guard| guard.value().to_owned())
    }

    pub async fn add_link(&self, discord_id: u64, osu_name: impl Into<String>) -> BotResult<()> {
        let name = osu_name.into();
        self.clients
            .psql
            .add_discord_link(discord_id, &name)
            .await?;
        self.data.discord_links.insert(discord_id, name);
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

    pub fn config_authorities_collect<F, T>(&self, guild_id: GuildId, f: F) -> Vec<T>
    where
        F: FnMut(u64) -> T,
    {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.authorities.iter().copied().map(f).collect()
    }

    pub fn config_prefixes(&self, guild_id: GuildId) -> Vec<String> {
        let config = self.data.guilds.entry(guild_id).or_default();
        config.prefixes.clone()
    }

    pub fn config_first_prefix(&self, guild_id: Option<GuildId>) -> String {
        match guild_id {
            Some(guild_id) => {
                let config = self.data.guilds.entry(guild_id).or_default();
                config.prefixes[0].clone()
            }
            None => "<".to_owned(),
        }
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

    pub fn pp(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.data.stored_values.mania_pp,
            GameMode::CTB => &self.data.stored_values.ctb_pp,
            _ => unreachable!(),
        }
    }

    pub fn add_role_assign(&self, channel_id: ChannelId, msg_id: MessageId, role_id: RoleId) {
        self.data
            .role_assigns
            .insert((channel_id.0, msg_id.0), role_id.0);
    }

    pub fn get_role_assign(&self, reaction: &Reaction) -> Option<RoleId> {
        self.data
            .role_assigns
            .get(&(reaction.channel_id.0, reaction.message_id.0))
            .map(|guard| RoleId(*guard.value()))
    }

    pub fn stars(&self, mode: GameMode) -> &Values {
        match mode {
            GameMode::MNA => &self.data.stored_values.mania_stars,
            GameMode::CTB => &self.data.stored_values.ctb_stars,
            _ => unreachable!(),
        }
    }

    pub fn add_tracking(&self, twitch_id: u64, channel_id: u64) {
        self.data
            .tracked_streams
            .entry(twitch_id)
            .or_default()
            .push(channel_id);
    }

    pub fn remove_tracking(&self, twitch_id: u64, channel_id: u64) {
        self.data
            .tracked_streams
            .entry(twitch_id)
            .and_modify(|channels| {
                if let Some(idx) = channels.iter().position(|&id| id == channel_id) {
                    channels.remove(idx);
                };
            });
    }

    pub fn tracked_users(&self) -> Vec<u64> {
        self.data
            .tracked_streams
            .iter()
            .map(|guard| *guard.key())
            .collect()
    }

    pub fn tracked_channels_for(&self, twitch_id: u64) -> Option<Vec<ChannelId>> {
        self.data.tracked_streams.get(&twitch_id).map(|guard| {
            guard
                .value()
                .iter()
                .map(|&channel| ChannelId(channel))
                .collect()
        })
    }

    pub fn tracked_users_in(&self, channel: ChannelId) -> Vec<u64> {
        self.data
            .tracked_streams
            .iter()
            .filter_map(|guard| {
                if guard.value().contains(&channel.0) {
                    Some(*guard.key())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn pp_lock(&self) -> &Mutex<()> {
        &self.data.perf_calc_mutex
    }

    pub fn add_game_and_start(
        &self,
        ctx: Arc<Context>,
        channel: ChannelId,
        mapsets: Vec<MapsetTagWrapper>,
    ) {
        if self.data.bg_games.get(&channel).is_some() {
            self.data.bg_games.remove(&channel);
        }
        self.data
            .bg_games
            .entry(channel)
            .or_insert_with(GameWrapper::new)
            .start(ctx, channel, mapsets);
    }

    pub async fn stop_and_remove_game(&self, channel: ChannelId) -> BotResult<()> {
        if let Some(mut game) = self.data.bg_games.get_mut(&channel) {
            game.stop().await?;
        }
        self.data.bg_games.remove(&channel);
        Ok(())
    }

    pub async fn game_hint(&self, channel: ChannelId) -> Result<String, BgGameError> {
        match self.data.bg_games.get_mut(&channel) {
            Some(game) => match game.hint().await? {
                Some(hint) => Ok(hint),
                None => Err(BgGameError::NotStarted),
            },
            None => Err(BgGameError::NoGame),
        }
    }

    pub async fn game_bigger(&self, channel: ChannelId) -> Result<Vec<u8>, BgGameError> {
        match self.data.bg_games.get_mut(&channel) {
            Some(mut game) => match game.sub_image().await? {
                Some(img) => Ok(img),
                None => Err(BgGameError::NotStarted),
            },
            None => Err(BgGameError::NoGame),
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
