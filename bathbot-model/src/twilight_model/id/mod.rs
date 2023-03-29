use std::num::NonZeroU64;

use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Fallible,
};
use twilight_model::id::Id;

mod niche;

pub use self::niche::IdNiche;

pub struct IdRkyv;

impl<T> ArchiveWith<Id<T>> for IdRkyv {
    type Archived = Id<T>;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &Id<T>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let out = out.cast::<NonZeroU64>();
        Archive::resolve(&field.into_nonzero(), pos, resolver, out);
    }
}

impl<T, S: Fallible + ?Sized> SerializeWith<Id<T>, S> for IdRkyv {
    #[inline]
    fn serialize_with(_: &Id<T>, _: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(())
    }
}

impl<T, D: Fallible + ?Sized> DeserializeWith<NonZeroU64, Id<T>, D> for IdRkyv {
    #[inline]
    fn deserialize_with(value: &NonZeroU64, _: &mut D) -> Result<Id<T>, <D as Fallible>::Error> {
        Ok(unsafe { Id::new_unchecked(value.get()) })
    }
}
