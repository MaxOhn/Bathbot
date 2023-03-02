#[cfg(debug_assertions)]
pub(crate) use self::debug::*;

#[cfg(not(debug_assertions))]
pub(crate) use self::release::*;

#[cfg(debug_assertions)]
/// Tracks required scratch space size and compares it with pre-set value
mod debug {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use eyre::{Result, WrapErr};
    use once_cell::sync::OnceCell;
    use rkyv::{
        de::deserializers::SharedDeserializeMap,
        ser::{
            serializers::{
                AlignedSerializer, AllocScratch, AllocSerializer, CompositeSerializer,
                FallbackScratch, HeapScratch, ScratchTracker,
            },
            Serializer,
        },
        AlignedVec, Serialize,
    };
    use twilight_model::{
        channel::Channel,
        guild::Role,
        user::{CurrentUser, User},
    };

    use crate::{
        model::{CachedGuild, CachedMember},
        serializer::{
            CHANNEL_SCRATCH_SIZE, CURRENT_USER_SCRATCH_SIZE, GUILD_SCRATCH_SIZE,
            MEMBER_SCRATCH_SIZE, ROLE_SCRATCH_SIZE, USER_SCRATCH_SIZE,
        },
    };

    static CHANNEL_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static CURRENT_USER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static GUILD_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static MEMBER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static ROLE_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static USER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();

    type InnerSerializer<const N: usize> = CompositeSerializer<
        AlignedSerializer<AlignedVec>,
        ScratchTracker<FallbackScratch<HeapScratch<N>, AllocScratch>>,
        SharedDeserializeMap,
    >;

    #[derive(Default)]
    pub(crate) struct SingleSerializer {
        serialization_count: AtomicUsize,
        accum_min_buffer_size: AtomicUsize,
    }

    impl SingleSerializer {
        pub(crate) fn any<T, const N: usize>(value: &T) -> Result<AlignedVec>
        where
            T: Serialize<AllocSerializer<N>>,
        {
            rkyv::to_bytes(value).wrap_err("Failed to serialize value")
        }

        pub(crate) fn channel(channel: &Channel) -> Result<AlignedVec> {
            Self::to_bytes::<_, CHANNEL_SCRATCH_SIZE>(channel, &CHANNEL_TRACKER, "channel")
                .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn current_user(user: &CurrentUser) -> Result<AlignedVec> {
            Self::to_bytes::<_, CURRENT_USER_SCRATCH_SIZE>(
                user,
                &CURRENT_USER_TRACKER,
                "current user",
            )
            .wrap_err("Failed to serialize current user")
        }

        pub(crate) fn guild(guild: &CachedGuild<'_>) -> Result<AlignedVec> {
            Self::to_bytes::<_, GUILD_SCRATCH_SIZE>(guild, &GUILD_TRACKER, "guild")
                .wrap_err("Failed to serialize guild")
        }

        pub(crate) fn member(member: &CachedMember<'_>) -> Result<AlignedVec> {
            Self::to_bytes::<_, MEMBER_SCRATCH_SIZE>(member, &MEMBER_TRACKER, "member")
                .wrap_err("Failed to serialize member")
        }

        pub(crate) fn role(role: &Role) -> Result<AlignedVec> {
            Self::to_bytes::<_, ROLE_SCRATCH_SIZE>(role, &ROLE_TRACKER, "role")
                .wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(user: &User) -> Result<AlignedVec> {
            Self::to_bytes::<_, USER_SCRATCH_SIZE>(user, &USER_TRACKER, "user")
                .wrap_err("Failed to serialize user")
        }

        fn update_min_buffer_size(&self, min_buffer_size: usize, kind: &str, allocated: usize) {
            let serialization_count = 1 + self.serialization_count.fetch_add(1, Ordering::Relaxed);

            let accum_min_buffer_size = min_buffer_size
                + self
                    .accum_min_buffer_size
                    .fetch_add(min_buffer_size, Ordering::Relaxed);

            if serialization_count >= 10 {
                let avg_min_buffer_size = accum_min_buffer_size / serialization_count;

                if allocated < avg_min_buffer_size {
                    tracing::warn!(
                        "Allocated {allocated} byte(s) to serialize {kind} but \
                        the average min buffer size was {avg_min_buffer_size}"
                    );
                }
            }
        }

        fn to_bytes<T, const N: usize>(
            value: &T,
            tracker: &OnceCell<Self>,
            kind: &str,
        ) -> Result<AlignedVec>
        where
            T: Serialize<InnerSerializer<N>>,
        {
            let mut serializer = InnerSerializer::new(
                Default::default(),
                ScratchTracker::new(Default::default()),
                Default::default(),
            );

            serializer.serialize_value(value)?;

            let (serializer, scratch, _) = serializer.into_components();

            tracker
                .get_or_init(Default::default)
                .update_min_buffer_size(scratch.min_buffer_size(), kind, N);

            Ok(serializer.into_inner())
        }
    }
}

#[cfg(not(debug_assertions))]
/// Straight serialization
mod release {
    use eyre::{Result, WrapErr};
    use rkyv::AlignedVec;
    use twilight_model::{
        channel::Channel,
        guild::Role,
        user::{CurrentUser, User},
    };

    use crate::model::{CachedGuild, CachedMember};

    use super::*;

    pub(crate) struct CacheSerializer;

    impl CacheSerializer {
        pub(crate) fn any<T, const N: usize>(value: &T) -> Result<AlignedVec>
        where
            T: Serialize<AllocSerializer<N>>,
        {
            rkyv::to_bytes(value).wrap_err("Failed to serialize value")
        }

        pub(crate) fn channel(channel: &Channel) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, CHANNEL_SCRATCH_SIZE>(channel)
                .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn current_user(user: &CurrentUser) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, CURRENT_USER_SCRATCH_SIZE>(user)
                .wrap_err("Failed to serialize current user")
        }

        pub(crate) fn guild(guild: &CachedGuild<'_>) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, GUILD_SCRATCH_SIZE>(guild).wrap_err("Failed to serialize guild")
        }

        pub(crate) fn member(member: &CachedMember<'_>) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, MEMBER_SCRATCH_SIZE>(member).wrap_err("Failed to serialize member")
        }

        pub(crate) fn role(role: &Role) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, ROLE_SCRATCH_SIZE>(role).wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(user: &User) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, USER_SCRATCH_SIZE>(user).wrap_err("Failed to serialize user")
        }
    }
}
