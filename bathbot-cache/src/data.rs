use std::time::Duration;

use rkyv::{
    bytecheck::CheckBytes,
    rancor::{BoxedError, Strategy},
    ser::{allocator::ArenaHandle, Serializer, WriterExt},
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    with::{SerializeWith, With},
    Archived, Portable, Serialize,
};

pub type BathbotRedisValidator<'a> = Strategy<Validator<ArchiveValidator<'a>, ()>, BoxedError>;

pub type BathbotRedisSerializer<'a> =
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

pub trait BathbotRedisData {
    const EXPIRE: Option<Duration>;

    type Archived: Portable + for<'a> CheckBytes<BathbotRedisValidator<'a>>;
    type Original: ?Sized;
    type With: for<'a> SerializeWith<
        Self::Original,
        BathbotRedisSerializer<'a>,
        Archived = Self::Archived,
    >;

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError>;
}
