use std::{marker::PhantomData, ops::Deref};

use eyre::{Report, Result};
use rkyv::{
    bytecheck::CheckBytes,
    rancor::{BoxedError, Strategy},
    seal::Seal,
    ser::{allocator::ArenaHandle, Serializer, WriterExt},
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    with::With,
    Archived, Portable, Serialize,
};

/// Similar to [`bathbot_cache::model::CachedArchive`] but generic over the
/// *archived* type rather than the original one.
///
/// In the future, this type should be used for all cache interactions.
pub struct CachedArchive<T> {
    bytes: AlignedVec<8>,
    phantom: PhantomData<T>,
}

impl<T> CachedArchive<T>
where
    T: Portable + for<'a> CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, BoxedError>>,
{
    pub fn new(bytes: AlignedVec<8>) -> Result<Self> {
        debug_assert!(align_of::<T>() <= 8);

        let slice = bytes.as_slice();
        let mut validator = Validator::new(ArchiveValidator::new(slice), ());
        rkyv::api::access_with_context::<T, _, _>(slice, &mut validator).map_err(Report::new)?;

        Ok(Self::new_unchecked(bytes))
    }
}

impl<T> CachedArchive<T> {
    const fn new_unchecked(bytes: AlignedVec<8>) -> Self {
        Self {
            bytes,
            phantom: PhantomData,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl<T: Portable> Deref for CachedArchive<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { rkyv::access_unchecked::<T>(self.as_bytes()) }
    }
}

impl<T: Portable> CachedArchive<T> {
    pub fn mutate<F: FnOnce(Seal<'_, T>)>(&mut self, f: F) {
        f(unsafe { rkyv::api::access_unchecked_mut::<T>(&mut self.bytes) })
    }
}

// ----------------------------------------------------------------
// TODO: the following should probably be refactored somewhere else
// ----------------------------------------------------------------

type BathbotRedisSerializer<'a> =
    Strategy<Serializer<AlignedVec<8>, ArenaHandle<'a>, ()>, BoxedError>;

pub fn serialize_using_arena<T>(data: &T) -> Result<AlignedVec<8>, BoxedError>
where
    T: for<'a> Serialize<BathbotRedisSerializer<'a>>,
{
    rkyv::util::with_arena(|arena| {
        let mut serializer = Serializer::new(AlignedVec::new(), arena.acquire(), ());
        rkyv::api::serialize_using(data, Strategy::<_, BoxedError>::wrap(&mut serializer))?;

        Ok(serializer.into_writer())
    })
}

pub fn serialize_using_arena_and_with<T, W>(data: &T) -> Result<AlignedVec<8>, BoxedError>
where
    T: ?Sized,
    With<T, W>: for<'a> Serialize<BathbotRedisSerializer<'a>>,
{
    rkyv::util::with_arena(|arena| {
        let wrap = With::<T, W>::cast(data);
        let mut serializer = Serializer::new(AlignedVec::new(), arena.acquire(), ());
        let resolver = wrap.serialize(Strategy::wrap(&mut serializer))?;
        serializer.align_for::<Archived<With<T, W>>>()?;

        // SAFETY: A proper resolver is being used and the serializer has been
        // aligned
        unsafe { serializer.resolve_aligned(wrap, resolver)? };

        Ok(serializer.into_writer())
    })
}
