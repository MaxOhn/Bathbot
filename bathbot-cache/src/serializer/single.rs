#[cfg(debug_assertions)]
pub(crate) use self::debug::*;

#[cfg(not(debug_assertions))]
pub(crate) use self::release::*;

#[cfg(debug_assertions)]
/// Tracks required scratch space size and compares it with pre-set value
mod debug {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use bathbot_model::twilight_model::{
        channel::Channel,
        guild::{Guild, Member, Role},
        user::{CurrentUser, User},
    };
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
        with::{ArchiveWith, SerializeWith, With},
        AlignedVec, Serialize,
    };
    use twilight_model::{
        channel::Channel as TwChannel,
        guild::Role as TwRole,
        user::{CurrentUser as TwCurrentUser, User as TwUser},
    };

    use crate::serializer::{
        multi::MemberSerializer, CHANNEL_SCRATCH_SIZE, CURRENT_USER_SCRATCH_SIZE,
        GUILD_SCRATCH_SIZE, MEMBER_SCRATCH_SIZE, ROLE_SCRATCH_SIZE, USER_SCRATCH_SIZE,
    };

    static CHANNEL_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static CURRENT_USER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static GUILD_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static MEMBER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static ROLE_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();
    static USER_TRACKER: OnceCell<SingleSerializer> = OnceCell::new();

    pub(crate) type InnerSerializer<const N: usize> = CompositeSerializer<
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

        pub(crate) fn channel(channel: &TwChannel) -> Result<AlignedVec> {
            Self::to_bytes::<_, CHANNEL_SCRATCH_SIZE>(
                With::<_, Channel>::cast(channel),
                &CHANNEL_TRACKER,
                "channel",
            )
            .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn current_user(user: &TwCurrentUser) -> Result<AlignedVec> {
            Self::to_bytes::<_, CURRENT_USER_SCRATCH_SIZE>(
                With::<_, CurrentUser>::cast(user),
                &CURRENT_USER_TRACKER,
                "current user",
            )
            .wrap_err("Failed to serialize current user")
        }

        pub(crate) fn guild<G>(guild: &G) -> Result<AlignedVec>
        where
            Guild: ArchiveWith<G> + SerializeWith<G, InnerSerializer<GUILD_SCRATCH_SIZE>>,
        {
            Self::to_bytes::<_, GUILD_SCRATCH_SIZE>(
                With::<_, Guild>::cast(guild),
                &GUILD_TRACKER,
                "guild",
            )
            .wrap_err("Failed to serialize guild")
        }

        pub(crate) fn member<M>(member: &M) -> Result<AlignedVec>
        where
            Member: ArchiveWith<M> + SerializeWith<M, MemberSerializer>,
        {
            Self::to_bytes::<_, MEMBER_SCRATCH_SIZE>(
                With::<_, Member>::cast(member),
                &MEMBER_TRACKER,
                "member",
            )
            .wrap_err("Failed to serialize member")
        }

        pub(crate) fn role(role: &TwRole) -> Result<AlignedVec> {
            Self::to_bytes::<_, ROLE_SCRATCH_SIZE>(
                With::<_, Role>::cast(role),
                &ROLE_TRACKER,
                "role",
            )
            .wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(user: &TwUser) -> Result<AlignedVec> {
            Self::to_bytes::<_, USER_SCRATCH_SIZE>(
                With::<_, User>::cast(user),
                &USER_TRACKER,
                "user",
            )
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
                    tracing::debug!(
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
    use bathbot_model::twilight_model::{
        channel::Channel,
        guild::{Guild, Role},
        user::{CurrentUser, User},
    };
    use eyre::{Result, WrapErr};
    use rkyv::{
        ser::serializers::AllocSerializer,
        with::{ArchiveWith, SerializeWith, With},
        AlignedVec, Serialize,
    };
    use twilight_model::{
        channel::Channel as TwChannel,
        guild::Role as TwRole,
        user::{CurrentUser as TwCurrentUser, User as TwUser},
    };

    use crate::serializer::{
        CHANNEL_SCRATCH_SIZE, CURRENT_USER_SCRATCH_SIZE, GUILD_SCRATCH_SIZE, ROLE_SCRATCH_SIZE,
        USER_SCRATCH_SIZE,
    };

    pub(crate) struct SingleSerializer;

    impl SingleSerializer {
        pub(crate) fn any<T, const N: usize>(value: &T) -> Result<AlignedVec>
        where
            T: Serialize<AllocSerializer<N>>,
        {
            rkyv::to_bytes(value).wrap_err("Failed to serialize value")
        }

        pub(crate) fn channel(channel: &TwChannel) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, CHANNEL_SCRATCH_SIZE>(With::<_, Channel>::cast(channel))
                .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn current_user(user: &TwCurrentUser) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, CURRENT_USER_SCRATCH_SIZE>(With::<_, CurrentUser>::cast(user))
                .wrap_err("Failed to serialize current user")
        }

        pub(crate) fn guild<G>(guild: &G) -> Result<AlignedVec>
        where
            Guild: ArchiveWith<G> + SerializeWith<G, AllocSerializer<GUILD_SCRATCH_SIZE>>,
        {
            rkyv::to_bytes::<_, GUILD_SCRATCH_SIZE>(With::<_, Guild>::cast(guild))
                .wrap_err("Failed to serialize guild")
        }

        pub(crate) fn role(role: &TwRole) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, ROLE_SCRATCH_SIZE>(With::<_, Role>::cast(role))
                .wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(user: &TwUser) -> Result<AlignedVec> {
            rkyv::to_bytes::<_, USER_SCRATCH_SIZE>(With::<_, User>::cast(user))
                .wrap_err("Failed to serialize user")
        }
    }
}
