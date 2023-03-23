#[cfg(debug_assertions)]
pub(crate) use debug::*;

#[cfg(not(debug_assertions))]
pub(crate) use release::*;

#[cfg(debug_assertions)]
mod debug {
    use eyre::Result;
    use rkyv::AlignedVec;
    use twilight_model::{channel::Channel, guild::Role, user::User};

    use crate::{model::CachedMember, serializer::single::SingleSerializer};

    #[derive(Default)]
    pub(crate) struct MultiSerializer;

    impl MultiSerializer {
        pub(crate) fn channel(&mut self, channel: &Channel) -> Result<AlignedVec> {
            SingleSerializer::channel(channel)
        }

        pub(crate) fn member(&mut self, member: &CachedMember<'_>) -> Result<AlignedVec> {
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
    use eyre::{Result, WrapErr};
    use rkyv::{
        ser::{serializers::AllocSerializer, Serializer},
        AlignedVec, Serialize,
    };
    use twilight_model::{channel::Channel, guild::Role, user::User};

    use crate::{
        model::CachedMember,
        serializer::{
            CHANNEL_SCRATCH_SIZE, MEMBER_SCRATCH_SIZE, ROLE_SCRATCH_SIZE, USER_SCRATCH_SIZE,
        },
    };

    const MAX_SCRATCH_SIZE: usize = max_scratch_size();

    #[derive(Default)]
    pub(crate) struct MultiSerializer {
        inner: Option<AllocSerializer<MAX_SCRATCH_SIZE>>,
    }

    impl MultiSerializer {
        pub(crate) fn channel(&mut self, channel: &Channel) -> Result<AlignedVec> {
            self.to_bytes_mut(channel)
                .wrap_err("Failed to serialize channel")
        }

        pub(crate) fn member(&mut self, member: &CachedMember<'_>) -> Result<AlignedVec> {
            self.to_bytes_mut(member)
                .wrap_err("Failed to serialize member")
        }

        pub(crate) fn role(&mut self, role: &Role) -> Result<AlignedVec> {
            self.to_bytes_mut(role).wrap_err("Failed to serialize role")
        }

        pub(crate) fn user(&mut self, user: &User) -> Result<AlignedVec> {
            self.to_bytes_mut(user).wrap_err("Failed to serialize user")
        }

        fn to_bytes_mut<T>(&mut self, value: &T) -> Result<AlignedVec>
        where
            T: Serialize<AllocSerializer<MAX_SCRATCH_SIZE>>,
        {
            let mut serializer = self.inner.take().unwrap_or_default();

            serializer.serialize_value(value)?;
            let (serializer, scratch, shard) = serializer.into_components();

            let _ = self
                .inner
                .insert(AllocSerializer::new(Default::default(), scratch, shard));

            Ok(serializer.into_inner())
        }
    }

    const fn max_scratch_size() -> usize {
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
