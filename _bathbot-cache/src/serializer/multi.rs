#[cfg(debug_assertions)]
pub(crate) use debug::*;
#[cfg(not(debug_assertions))]
pub(crate) use release::*;

#[cfg(debug_assertions)]
mod debug {
    use bathbot_model::twilight_model::guild::Member;
    use eyre::Result;
    use rkyv::{
        with::{ArchiveWith, SerializeWith},
        AlignedVec,
    };
    use twilight_model::{channel::Channel, guild::Role, user::User};

    use crate::serializer::{single::SingleSerializer, CacheSerializer, MEMBER_SCRATCH_SIZE};

    pub(crate) type MemberSerializer = CacheSerializer<MEMBER_SCRATCH_SIZE>;

    #[derive(Default)]
    pub(crate) struct MultiSerializer;

    impl MultiSerializer {
        pub(crate) fn channel(&mut self, channel: &Channel) -> Result<AlignedVec> {
            SingleSerializer::channel(channel)
        }

        pub(crate) fn member<M>(&mut self, member: &M) -> Result<AlignedVec>
        where
            Member: ArchiveWith<M> + SerializeWith<M, MemberSerializer>,
        {
            SingleSerializer::member(member)
        }

        pub(crate) fn role(&mut self, role: &Role) -> Result<AlignedVec> {
            SingleSerializer::role(role)
        }

        pub(crate) fn user(&mut self, user: &User) -> Result<AlignedVec> {
            SingleSerializer::user(user)
        }
    }
}

#[cfg(not(debug_assertions))]
mod release {
    use bathbot_model::twilight_model::{
        channel::Channel,
        guild::{Member, Role},
        user::User,
    };
    use eyre::{Result, WrapErr};
    use rkyv::{
        ser::Serializer,
        with::{ArchiveWith, SerializeWith, With},
        AlignedVec, Serialize,
    };
    use twilight_model::{
        channel::Channel as TwChannel, guild::Role as TwRole, user::User as TwUser,
    };

    use crate::serializer::{
        CacheSerializer, CHANNEL_SCRATCH_SIZE, MEMBER_SCRATCH_SIZE, ROLE_SCRATCH_SIZE,
        USER_SCRATCH_SIZE,
    };

    const MAX_SCRATCH_SIZE: usize = max_scratch_size();

    pub(crate) type MemberSerializer = CacheSerializer<MAX_SCRATCH_SIZE>;

    #[derive(Default)]
    pub(crate) struct MultiSerializer {
        inner: Option<CacheSerializer<MAX_SCRATCH_SIZE>>,
    }

    impl MultiSerializer {
        pub(crate) fn channel(&mut self, channel: &TwChannel) -> Result<AlignedVec> {
            self.to_bytes_mut(With::<_, Channel>::cast(channel))
                .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn member<M>(&mut self, member: &M) -> Result<AlignedVec>
        where
            Member: ArchiveWith<M> + SerializeWith<M, CacheSerializer<MAX_SCRATCH_SIZE>>,
        {
            self.to_bytes_mut(With::<_, Member>::cast(member))
                .wrap_err("Failed to serialize member")
        }

        pub(crate) fn role(&mut self, role: &TwRole) -> Result<AlignedVec> {
            self.to_bytes_mut(With::<_, Role>::cast(role))
                .wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(&mut self, user: &TwUser) -> Result<AlignedVec> {
            self.to_bytes_mut(With::<_, User>::cast(user))
                .wrap_err("Failed to serialize user")
        }

        fn to_bytes_mut<T>(&mut self, value: &T) -> Result<AlignedVec>
        where
            T: Serialize<CacheSerializer<MAX_SCRATCH_SIZE>>,
        {
            let mut serializer = self.inner.take().unwrap_or_default();

            serializer.serialize_value(value)?;
            let (serializer, scratch, shared) = serializer.into_components();

            let _ = self.inner.insert(CacheSerializer::new(scratch, shared));

            Ok(serializer.into_inner())
        }
    }

    const fn max_scratch_size() -> usize {
        #[allow(clippy::absurd_extreme_comparisons)]
        let mut size = if CHANNEL_SCRATCH_SIZE > MEMBER_SCRATCH_SIZE {
            CHANNEL_SCRATCH_SIZE
        } else {
            MEMBER_SCRATCH_SIZE
        };

        #[allow(clippy::absurd_extreme_comparisons)]
        if ROLE_SCRATCH_SIZE > size {
            size = ROLE_SCRATCH_SIZE;
        }

        #[allow(clippy::absurd_extreme_comparisons)]
        if USER_SCRATCH_SIZE > size {
            size = USER_SCRATCH_SIZE;
        }

        size
    }
}
