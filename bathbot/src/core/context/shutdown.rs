use bathbot_model::twilight::id::{ArchivedId, IdRkyv, IdRkyvMap};
use eyre::{Result, WrapErr};
use futures::stream::StreamExt;
use rkyv::{
    collections::util::{Entry, EntryAdapter},
    primitive::ArchivedU64,
    rancor::{BoxedError, Fallible, Strategy},
    ser::{Allocator, Serializer, Writer, WriterExt},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith, With},
    Archived, Place, Serialize,
};
use twilight_gateway::Shard;
use twilight_model::id::{marker::GuildMarker, Id};

use crate::{util::ChannelExt, Context};

impl Context {
    #[cold]
    pub async fn shutdown(shards: &mut [Shard]) {
        let this = Self::get();

        let scores_ws_disconnect = match this.scores_ws_disconnect.lock().unwrap().take() {
            Some(mut disconnect) => match disconnect.tx.take() {
                Some(tx) => {
                    let _: Result<_, _> = tx.send(());

                    disconnect.rx.take()
                }
                None => None,
            },
            None => None,
        };

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
            info!("Disconnecting from ordr");
            ordr.disconnect();
        }

        if let Some(rx) = scores_ws_disconnect {
            let _: Result<_, _> = rx.await;
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

        info!("Finished shutdown routine");
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

        let content = "The game will be aborted because I'm about to reboot, \
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
        let guild_shards: Vec<_> = self
            .guild_shards()
            .pin()
            .iter()
            .map(|(guild, shard)| (*guild, *shard))
            .collect();

        let len = guild_shards.len();
        info!(len, "Storing guild shards...");

        let bytes = rkyv::util::with_arena(|arena| {
            let wrap = With::<Original, CacheGuildShards>::cast(&guild_shards);
            let mut serializer = Serializer::new(Vec::new(), arena.acquire(), ());
            let resolver = wrap.serialize(Strategy::<_, BoxedError>::wrap(&mut serializer))?;
            WriterExt::<BoxedError>::align_for::<Archived<With<Original, CacheGuildShards>>>(
                &mut serializer,
            )?;

            // SAFETY: A proper resolver is being used and the serializer has been
            // aligned
            unsafe { WriterExt::<BoxedError>::resolve_aligned(&mut serializer, wrap, resolver)? };

            Ok::<_, BoxedError>(serializer.into_writer())
        })
        .wrap_err("Failed to serialize guild shards")?;

        Self::cache()
            .store_new_raw("guild_shards", &bytes, store_duration)
            .await
            .wrap_err("Failed to store in redis")?;

        Ok(len)
    }

    #[cold]
    async fn store_miss_analyzer_guilds(store_duration: u64) -> Result<usize> {
        info!(
            "[DEBUG] Miss analyzer guilds: {}",
            Self::miss_analyzer_guilds().len()
        );

        let miss_analyzer_guilds: Vec<_> =
            Self::miss_analyzer_guilds().pin().iter().copied().collect();

        let len = miss_analyzer_guilds.len();
        info!(len, "Storing miss analyzer guilds...");

        let bytes = rkyv::util::with_arena(|arena| {
            let slice = miss_analyzer_guilds.as_slice();
            let wrap = With::<&[Id<GuildMarker>], IdRkyvMap>::cast(&slice);
            let mut serializer = Serializer::new(Vec::new(), arena.acquire(), ());
            let resolver = wrap.serialize(Strategy::<_, BoxedError>::wrap(&mut serializer))?;
            WriterExt::<BoxedError>::align_for::<Archived<With<&[Id<GuildMarker>], IdRkyvMap>>>(
                &mut serializer,
            )?;

            // SAFETY: A proper resolver is being used and the serializer has been
            // aligned
            unsafe { WriterExt::<BoxedError>::resolve_aligned(&mut serializer, wrap, resolver)? };

            Ok::<_, BoxedError>(serializer.into_writer())
        })
        .wrap_err("Failed to serialize miss analyzer guilds")?;

        Self::cache()
            .store_new_raw("miss_analyzer_guilds", &bytes, store_duration)
            .await?;

        Ok(len)
    }
}

// TODO: clean this up
pub struct CacheGuildShards;

type Original = [(Id<GuildMarker>, u64)];
type ArchivedCacheGuildShards = ArchivedVec<Entry<ArchivedId<GuildMarker>, ArchivedU64>>;

impl ArchiveWith<Original> for CacheGuildShards {
    type Archived = ArchivedCacheGuildShards;
    type Resolver = VecResolver;

    fn resolve_with(field: &Original, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(field.len(), resolver, out);
    }
}

impl<S: Fallible + Allocator + Writer + ?Sized> SerializeWith<Original, S> for CacheGuildShards {
    fn serialize_with(field: &Original, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        let iter = field.iter().map(|(guild, shard)| {
            EntryAdapter::<_, _, With<_, IdRkyv>, u64>::new(With::<_, IdRkyv>::cast(guild), shard)
        });

        ArchivedVec::serialize_from_iter(iter, serializer)
    }
}

impl<D: Fallible + ?Sized>
    DeserializeWith<ArchivedCacheGuildShards, Vec<Entry<Id<GuildMarker>, u64>>, D>
    for CacheGuildShards
{
    fn deserialize_with(
        field: &ArchivedCacheGuildShards,
        _: &mut D,
    ) -> Result<Vec<Entry<Id<GuildMarker>, u64>>, D::Error> {
        Ok(field
            .iter()
            .map(|entry| Entry {
                key: entry.key.to_native(),
                value: entry.value.to_native(),
            })
            .collect())
    }
}
