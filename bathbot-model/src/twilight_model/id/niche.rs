use rkyv::{
    niche::option_nonzero::ArchivedOptionNonZeroU64,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Fallible,
};

use super::Id;

type ArchivedOptionId = ArchivedOptionNonZeroU64;

pub struct IdNiche;

impl<T> ArchiveWith<Option<Id<T>>> for IdNiche {
    type Archived = ArchivedOptionId;
    type Resolver = ();

    #[inline]
    #[allow(unsafe_code)]
    unsafe fn resolve_with(
        field: &Option<Id<T>>,
        _: usize,
        _: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        ArchivedOptionId::resolve_from_option(field.map(Id::into_nonzero), out);
    }
}

impl<S: Fallible + ?Sized, T> SerializeWith<Option<Id<T>>, S> for IdNiche {
    #[inline]
    fn serialize_with(_: &Option<Id<T>>, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible + ?Sized, T> DeserializeWith<ArchivedOptionId, Option<Id<T>>, D> for IdNiche {
    #[inline]
    fn deserialize_with(field: &ArchivedOptionId, _: &mut D) -> Result<Option<Id<T>>, D::Error> {
        Ok(field
            .as_ref()
            .map(|id| unsafe { Id::new_unchecked(id.get()) }))
    }
}
