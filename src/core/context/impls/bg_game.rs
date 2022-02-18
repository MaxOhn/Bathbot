use std::sync::Arc;

use dashmap::mapref::entry::Entry;
use eyre::Report;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    bg_game::{BgGameError, GameWrapper},
    database::MapsetTagWrapper,
    BotResult, Context,
};

impl Context {
    pub async fn add_game_and_start(
        this: Arc<Context>,
        channel: Id<ChannelMarker>,
        mapsets: Vec<MapsetTagWrapper>,
    ) {
        if this.data.bg_games.get(&channel).is_some() {
            this.data.bg_games.remove(&channel);
        }

        let game = GameWrapper::new(Arc::clone(&this), channel, mapsets).await;

        match this.data.bg_games.entry(channel) {
            Entry::Occupied(mut e) => {
                if let Err(err) = e.get().stop() {
                    let report = Report::new(err)
                        .wrap_err("failed to stop existing game that's about to be overwritten");
                    warn!("{:?}", report);
                }

                e.insert(game);
            }
            Entry::Vacant(e) => {
                e.insert(game);
            }
        }
    }

    pub fn has_running_game(&self, channel: Id<ChannelMarker>) -> bool {
        self.data
            .bg_games
            .iter()
            .any(|guard| *guard.key() == channel)
    }

    pub fn game_channels(&self) -> Vec<Id<ChannelMarker>> {
        self.data
            .bg_games
            .iter()
            .map(|guard| *guard.key())
            .collect()
    }

    pub fn restart_game(&self, channel: Id<ChannelMarker>) -> BotResult<bool> {
        match self.data.bg_games.get(&channel) {
            Some(game) => Ok(game.restart().map(|_| true)?),
            None => Ok(false),
        }
    }

    pub fn stop_game(&self, channel: Id<ChannelMarker>) -> BotResult<bool> {
        if self.data.bg_games.contains_key(&channel) {
            if let Some(game) = self.data.bg_games.get(&channel) {
                game.stop()?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn remove_game(&self, channel: Id<ChannelMarker>) {
        self.data.bg_games.remove(&channel);
    }

    pub async fn game_hint(&self, channel: Id<ChannelMarker>) -> Result<String, BgGameError> {
        match self.data.bg_games.get(&channel) {
            Some(game) => Ok(game.hint().await),
            None => Err(BgGameError::NoGame),
        }
    }

    pub async fn game_bigger(&self, channel: Id<ChannelMarker>) -> Result<Vec<u8>, BgGameError> {
        match self.data.bg_games.get(&channel) {
            Some(game) => game.sub_image().await,
            None => Err(BgGameError::NoGame),
        }
    }
}
