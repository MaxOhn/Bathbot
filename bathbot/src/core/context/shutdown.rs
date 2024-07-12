use bathbot_model::twilight_model::id::IdRkyv;
use eyre::{Result, WrapErr};
use futures::stream::StreamExt;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    vec::ArchivedVec,
    with::With,
    Archive,
};
use twilight_gateway::Shard;
use twilight_model::id::{marker::GuildMarker, Id};

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

        if let Err(err) = Context::cache().freeze(&resume_data).await {
            error!(?err, "Failed to freeze cache");
        }

        const STORE_DURATION: u64 = 240;

        match this.store_guild_shards(STORE_DURATION).await {
            Ok(len) => info!("Stored {len} guild shards"),
            Err(err) => error!(?err, "Failed to store guild shards"),
        }

        match Context::store_miss_analyzer_guilds(STORE_DURATION).await {
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

    /// Serialize guild shards and store them in redis for 240 seconds
    #[cold]
    async fn store_guild_shards(&self, store_duration: u64) -> Result<usize> {
        let mut serializer = AllocSerializer::<0>::default();

        // Will be serialized as ArchivedVec
        let guild_shards = self.guild_shards().pin();
        let len = guild_shards.len();

        // Serialize data
        for (guild, shard) in guild_shards.iter() {
            serializer
                .serialize_value(With::<_, IdRkyv>::cast(guild))
                .wrap_err("Failed to serialize guild")?;

            serializer
                .serialize_value(shard)
                .wrap_err("Failed to serialize shard")?;
        }

        type ArchivedData =
            ArchivedVec<(<With<Id<GuildMarker>, IdRkyv> as Archive>::Archived, u64)>;

        // Align buffer
        serializer
            .align_for::<ArchivedData>()
            .wrap_err("Failed to align serializer")?;

        Self::finalize_store_as_vec(serializer, len, "guild_shards", store_duration).await
    }

    #[cold]
    async fn store_miss_analyzer_guilds(store_duration: u64) -> Result<usize> {
        let mut serializer = AllocSerializer::<0>::default();

        // Will be serialized as ArchivedVec
        let miss_analyzer_guilds = Self::miss_analyzer_guilds().pin();
        let len = miss_analyzer_guilds.len();

        // Serialize data
        for guild in miss_analyzer_guilds.iter() {
            serializer
                .serialize_value(With::<_, IdRkyv>::cast(guild))
                .wrap_err("Failed to serialize guild")?;
        }

        type ArchivedData = ArchivedVec<<With<Id<GuildMarker>, IdRkyv> as Archive>::Archived>;

        // Align buffer
        serializer
            .align_for::<ArchivedData>()
            .wrap_err("Failed to align serializer")?;

        Self::finalize_store_as_vec(serializer, len, "miss_analyzer_guilds", store_duration).await
    }

    // Does not include serializer alignment to avoid generics
    async fn finalize_store_as_vec<const N: usize>(
        mut serializer: AllocSerializer<N>,
        len: usize,
        key: &str,
        duration: u64,
    ) -> Result<usize> {
        // Serialize relative pointer
        for byte in (-(serializer.pos() as i32)).to_le_bytes() {
            serializer
                .serialize_value(&byte)
                .wrap_err("Failed to serialize rel ptr")?;
        }

        // Serialize length
        serializer
            .serialize_value(&len)
            .wrap_err("Failed to serialize length")?;

        let bytes = serializer.into_serializer().into_inner();

        // Store bytes
        Context::cache()
            .store_new_raw(key, &bytes, duration)
            .await
            .wrap_err("Failed to store in redis")?;

        Ok(len)
    }
}
