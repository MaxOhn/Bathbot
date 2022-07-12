use std::{
    hash::Hash,
    iter::Take,
    mem::{self, MaybeUninit},
    slice,
    time::Instant,
};

use bb8_redis::redis::AsyncCommands;
use futures::{future::Either, stream::FuturesUnordered, StreamExt};
use rkyv::{
    ser::{
        serializers::{AlignedSerializer, AllocSerializer},
        Serializer,
    },
    vec::ArchivedVec,
    Archive, ScratchVec, Serialize,
};
use twilight_cache_inmemory::iter::ResourceIter;

use super::{
    Cache, ColdResumeData, ColdResumeErrorKind, FreezeError, FreezeInnerError, Redis, ResumeData,
    CHANNEL_KEY_PREFIX, CURRENT_USER_KEY, DATA_KEY, GUILD_KEY_PREFIX, MEMBER_KEY_PREFIX,
    ROLE_KEY_PREFIX, STORE_DURATION, USER_KEY_PREFIX,
};

impl Cache {
    pub async fn freeze(&self, redis: &Redis, resume_data: ResumeData) -> Result<(), FreezeError> {
        let start = Instant::now();
        let mut futs = FuturesUnordered::new();

        let fut = Either::Left(self.freeze_guilds(redis));
        futs.push(fut);

        let fut = Either::Right(Either::Left(self.freeze_users(redis)));
        futs.push(fut);

        let fut = Either::Right(Either::Right(Either::Left(self.freeze_members(redis))));
        futs.push(fut);

        let fut = Either::Right(Either::Right(Either::Right(Either::Left(
            self.freeze_roles(redis),
        ))));
        futs.push(fut);

        let fut = Either::Right(Either::Right(Either::Right(Either::Right(Either::Left(
            self.freeze_channels(redis),
        )))));
        futs.push(fut);

        let fut = Either::Right(Either::Right(Either::Right(Either::Right(Either::Right(
            self.freeze_current_user(redis),
        )))));
        futs.push(fut);

        let mut guild_chunks = 0;
        let mut user_chunks = 0;
        let mut member_chunks = 0;
        let mut role_chunks = 0;
        let mut channel_chunks = 0;

        while let Some(count) = futs.next().await.transpose()? {
            match count {
                FreezeChunkCount::Channels(count) => channel_chunks = count,
                FreezeChunkCount::CurrentUser => {}
                FreezeChunkCount::Guilds(count) => guild_chunks = count,
                FreezeChunkCount::Members(count) => member_chunks = count,
                FreezeChunkCount::Roles(count) => role_chunks = count,
                FreezeChunkCount::Users(count) => user_chunks = count,
            }
        }

        let data = ColdResumeData {
            resume_data,
            guild_chunks,
            user_chunks,
            member_chunks,
            channel_chunks,
            role_chunks,
        };

        let bytes = rkyv::to_bytes::<_, 1024>(&data).map_err(|inner| FreezeError {
            kind: ColdResumeErrorKind::ResumeData,
            inner: FreezeInnerError::from(inner),
        })?;

        trace!("Resume data bytes: {}", bytes.len());

        Self::store_bytes(DATA_KEY, &bytes, redis)
            .await
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::ResumeData,
                inner,
            })?;

        info!("Successfully froze cache [{:?}]", start.elapsed());

        Ok(())
    }

    async fn freeze_current_user(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_current_user_(redis)
            .await
            .map(|_| FreezeChunkCount::CurrentUser)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::CurrentUser,
                inner,
            })
    }

    async fn freeze_current_user_(&self, redis: &Redis) -> Result<(), FreezeInnerError> {
        // ~56 bytes
        let bytes = self
            .current_user(rkyv::to_bytes::<_, 64>)
            .map_err(|_| FreezeInnerError::MissingCurrentUser)??;

        trace!("Current user bytes: {}", bytes.len());
        Self::store_bytes(CURRENT_USER_KEY, &bytes, redis).await?;

        Ok(())
    }

    async fn freeze_channels(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_channels_(redis)
            .await
            .map(FreezeChunkCount::Channels)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::Channels,
                inner,
            })
    }

    async fn freeze_channels_(&self, redis: &Redis) -> Result<usize, FreezeInnerError> {
        let mut channels_count = self.inner.stats().channels_total();
        debug!("Freezing {channels_count} channels");
        let mut channels = self.inner.iter().channels();
        let mut idx = 0;

        const CHANNELS_CHUNK_SIZE: usize = 80_000;
        const SERIALIZER_SIZE: usize = 16_777_216;
        let mut serializer = AllocSerializer::<SERIALIZER_SIZE>::default();

        while channels_count > 0 {
            let count = CHANNELS_CHUNK_SIZE.min(channels_count);
            channels_count -= count;
            let iter = (&mut channels).take(count);

            // ~170 bytes/channel
            Self::serialize_content::<_, _, SERIALIZER_SIZE>(iter, count, &mut serializer)?;

            let (serializer_, scratch, shared) = serializer.into_components();
            let mut bytes = serializer_.into_inner();

            trace!("Channel bytes: {} [{idx}]", bytes.len());
            let key = format!("{CHANNEL_KEY_PREFIX}_{idx}");
            Self::store_bytes(&key, &bytes, redis).await?;
            idx += 1;

            bytes.clear();
            let serializer_ = AlignedSerializer::new(bytes);
            serializer = AllocSerializer::new(serializer_, scratch, shared);
        }

        Ok(idx)
    }

    async fn freeze_roles(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_roles_(redis)
            .await
            .map(FreezeChunkCount::Roles)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::Roles,
                inner,
            })
    }

    async fn freeze_roles_(&self, redis: &Redis) -> Result<usize, FreezeInnerError> {
        let mut roles_count = self.inner.stats().roles();
        debug!("Freezing {roles_count} roles");
        let mut roles = self.inner.iter().roles();
        let mut idx = 0;

        const ROLES_CHUNK_SIZE: usize = 150_000;
        const SERIALIZER_SIZE: usize = 8_388_608;
        let mut serializer = AllocSerializer::<SERIALIZER_SIZE>::default();

        while roles_count > 0 {
            let count = ROLES_CHUNK_SIZE.min(roles_count);
            roles_count -= count;
            let iter = (&mut roles).take(count);

            // ~49 bytes/role
            Self::serialize_content::<_, _, 8_388_608>(iter, count, &mut serializer)?;

            let (serializer_, scratch, shared) = serializer.into_components();
            let mut bytes = serializer_.into_inner();

            trace!("Role bytes: {} [{idx}]", bytes.len());
            let key = format!("{ROLE_KEY_PREFIX}_{idx}");
            Self::store_bytes(&key, &bytes, redis).await?;
            idx += 1;

            bytes.clear();
            let serializer_ = AlignedSerializer::new(bytes);
            serializer = AllocSerializer::new(serializer_, scratch, shared);
        }

        Ok(idx)
    }

    async fn freeze_members(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_members_(redis)
            .await
            .map(FreezeChunkCount::Members)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::Members,
                inner,
            })
    }

    async fn freeze_members_(&self, redis: &Redis) -> Result<usize, FreezeInnerError> {
        let mut members_count = self.inner.stats().members();
        debug!("Freezing {members_count} members");
        let mut members = self.inner.iter().members();
        let mut idx = 0;

        const MEMBERS_CHUNK_SIZE: usize = 150_000;
        const SERIALIZER_SIZE: usize = 16_777_216;
        let mut serializer = AllocSerializer::<SERIALIZER_SIZE>::default();

        while members_count > 0 {
            let count = MEMBERS_CHUNK_SIZE.min(members_count);
            members_count -= count;
            let iter = (&mut members).take(count);

            // ~91 bytes/member
            Self::serialize_content::<_, _, 16_777_216>(iter, count, &mut serializer)?;

            let (serializer_, scratch, shared) = serializer.into_components();
            let mut bytes = serializer_.into_inner();

            trace!("Member bytes: {} [{idx}]", bytes.len());
            let key = format!("{MEMBER_KEY_PREFIX}_{idx}");
            Self::store_bytes(&key, &bytes, redis).await?;
            idx += 1;

            bytes.clear();
            let serializer_ = AlignedSerializer::new(bytes);
            serializer = AllocSerializer::new(serializer_, scratch, shared);
        }

        Ok(idx)
    }

    async fn freeze_users(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_users_(redis)
            .await
            .map(FreezeChunkCount::Users)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::Users,
                inner,
            })
    }

    async fn freeze_users_(&self, redis: &Redis) -> Result<usize, FreezeInnerError> {
        let mut users_count = self.inner.stats().users();
        debug!("Freezing {users_count} users");
        let mut users = self.inner.iter().users();
        let mut idx = 0;

        const USERS_CHUNK_SIZE: usize = 175_000;
        const SERIALIZER_SIZE: usize = 8_388_608;
        let mut serializer = AllocSerializer::<SERIALIZER_SIZE>::default();

        while users_count > 0 {
            let count = USERS_CHUNK_SIZE.min(users_count);
            users_count -= count;
            let iter = (&mut users).take(count);

            // ~45 bytes/user
            Self::serialize_content::<_, _, 8_388_608>(iter, count, &mut serializer)?;

            let (serializer_, scratch, shared) = serializer.into_components();
            let mut bytes = serializer_.into_inner();

            trace!("User bytes: {} [{idx}]", bytes.len());
            let key = format!("{USER_KEY_PREFIX}_{idx}");
            Self::store_bytes(&key, &bytes, redis).await?;
            idx += 1;

            bytes.clear();
            let serializer_ = AlignedSerializer::new(bytes);
            serializer = AllocSerializer::new(serializer_, scratch, shared);
        }

        Ok(idx)
    }

    async fn freeze_guilds(&self, redis: &Redis) -> Result<FreezeChunkCount, FreezeError> {
        self.freeze_guilds_(redis)
            .await
            .map(FreezeChunkCount::Guilds)
            .map_err(|inner| FreezeError {
                kind: ColdResumeErrorKind::Guilds,
                inner,
            })
    }

    async fn freeze_guilds_(&self, redis: &Redis) -> Result<usize, FreezeInnerError> {
        let mut guilds_count = self.inner.stats().guilds();
        debug!("Freezing {guilds_count} guilds");
        let mut guilds = self.inner.iter().guilds();
        let mut idx = 0;

        const GUILDS_CHUNK_SIZE: usize = 100_000;
        const SERIALIZER_SIZE: usize = 8_388_608;
        let mut serializer = AllocSerializer::<SERIALIZER_SIZE>::default();

        while guilds_count > 0 {
            let count = GUILDS_CHUNK_SIZE.min(guilds_count);
            guilds_count -= count;
            let iter = (&mut guilds).take(count);

            // ~78 bytes/guild
            Self::serialize_content::<_, _, 8_388_608>(iter, count, &mut serializer)?;

            let (serializer_, scratch, shared) = serializer.into_components();
            let mut bytes = serializer_.into_inner();

            trace!("Guild bytes: {} [{idx}]", bytes.len());
            let key = format!("{GUILD_KEY_PREFIX}_{idx}");
            Self::store_bytes(&key, &bytes, redis).await?;
            idx += 1;

            bytes.clear();
            let serializer_ = AlignedSerializer::new(bytes);
            serializer = AllocSerializer::new(serializer_, scratch, shared);
        }

        Ok(idx)
    }

    fn serialize_content<K, V, const N: usize>(
        iter: Take<&'_ mut ResourceIter<'_, K, V>>,
        len: usize,
        serializer: &mut AllocSerializer<N>,
    ) -> Result<(), FreezeInnerError>
    where
        K: Eq + Hash,
        V: Archive + Serialize<AllocSerializer<N>>,
    {
        let mut resolvers = unsafe { ScratchVec::new(serializer, len) }?;

        for elem in iter {
            let resolver = elem.value().serialize(serializer)?;
            resolvers.push((elem, resolver));
        }

        let pos = serializer.align_for::<V>()?;

        let resolver = unsafe {
            for (elem, resolver) in resolvers.drain(..) {
                serializer.resolve_aligned(elem.value(), resolver)?;
            }

            resolvers.free(serializer)?;

            mem::transmute(pos)
        };

        let mut resolved = MaybeUninit::<ArchivedVec<V>>::uninit();

        unsafe {
            resolved.as_mut_ptr().write_bytes(0, 1);
            ArchivedVec::resolve_from_len(len, serializer.pos(), resolver, resolved.as_mut_ptr());
        }

        let data = resolved.as_ptr().cast::<u8>();
        let len = mem::size_of::<ArchivedVec<V>>();
        unsafe { serializer.write(slice::from_raw_parts(data, len))? };

        Ok(())
    }

    async fn store_bytes(key: &str, bytes: &[u8], redis: &Redis) -> Result<(), FreezeInnerError> {
        redis
            .get()
            .await?
            .set_ex(key, bytes, STORE_DURATION)
            .await?;

        Ok(())
    }
}

enum FreezeChunkCount {
    Channels(usize),
    CurrentUser,
    Guilds(usize),
    Members(usize),
    Roles(usize),
    Users(usize),
}
