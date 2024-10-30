use std::time::Duration;

use bathbot_cache::bathbot::{
    guild_shards::CacheGuildShards, miss_analyzer::CacheMissAnalyzerGuilds,
};
use eyre::Result;
use futures::stream::StreamExt;
use twilight_gateway::Shard;

use crate::{util::ChannelExt, Context};

impl Context {
    #[cold]
    pub async fn shutdown(shards: &mut [Shard]) {
        let this = Self::get();

        // Disable tracking while preparing shutdown
        #[cfg(feature = "osutracking")]
        Context::tracking().set_stop_tracking(true);

        // Prevent non-minimized msgs from getting minimized
        this.active_msgs.clear().await;

        let count = Context::stop_all_games().await;
        info!("Stopped {count} bg games");

        #[cfg(feature = "matchlive")]
        {
            let count = this.notify_match_live_shutdown().await;
            info!("Stopped match tracking in {count} channels");
        }

        if let Some(ordr) = Context::ordr() {
            ordr.disconnect();
        }

        let resume_data = Self::down_resumable(shards).await;
        let expire = Duration::from_secs(240);

        if let Err(err) = Context::cache().freeze(&resume_data, Some(expire)).await {
            error!(?err, "Failed to freeze cache");
        }

        match this.store_guild_shards().await {
            Ok(len) => info!("Stored {len} guild shards"),
            Err(err) => error!(?err, "Failed to store guild shards"),
        }

        match Context::store_miss_analyzer_guilds().await {
            Ok(len) => info!("Stored {len} miss analyzer guilds"),
            Err(err) => error!(?err, "Failed to store miss analyzer guilds"),
        }
    }

    /// Notify all active bg games that they'll be aborted due to a bot restart
    #[cold]
    async fn stop_all_games() -> usize {
        let mut active_games = Vec::new();
        let mut stream = Context::bg_games().iter();

        while let Some(guard) = stream.next().await {
            let key = *guard.key();
            let value = guard.value().to_owned();

            active_games.push((key, value));
        }

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'll abort this game because I'm about to reboot, \
            you can start a new game again in just a moment...";

        for (channel, game) in active_games {
            match game.stop() {
                Ok(_) => {
                    let _ = channel.plain_message(content).await;
                    count += 1;
                }
                Err(err) => warn!(%channel, ?err, "Error while stopping game"),
            }
        }

        count
    }

    #[cold]
    async fn store_guild_shards(&self) -> Result<usize> {
        let guild_shards: Vec<_> = self
            .guild_shards()
            .pin()
            .iter()
            .map(|(guild, shard)| (*guild, *shard))
            .collect();

        let len = guild_shards.len();

        Self::cache()
            .store::<CacheGuildShards>("GUILD_SHARDS", &guild_shards)
            .await?;

        Ok(len)
    }

    #[cold]
    async fn store_miss_analyzer_guilds() -> Result<usize> {
        let miss_analyzer_guilds: Vec<_> =
            Self::miss_analyzer_guilds().pin().keys().copied().collect();
        let len = miss_analyzer_guilds.len();
        Self::cache()
            .store::<CacheMissAnalyzerGuilds>("MISS_ANALYZER_GUILDS", &miss_analyzer_guilds)
            .await?;

        Ok(len)
    }
}
