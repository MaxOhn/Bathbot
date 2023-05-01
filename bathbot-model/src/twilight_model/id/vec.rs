use std::{mem, num::NonZeroU64};

use rkyv::{
    boxed::{ArchivedBox, BoxResolver},
    ser::Serializer,
    with::{ArchiveWith, SerializeWith},
    ArchiveUnsized, Fallible,
};
use twilight_model::id::Id;

pub struct IdVec;

impl<T> ArchiveWith<Vec<Id<T>>> for IdVec {
    type Archived = ArchivedBox<<[NonZeroU64] as ArchiveUnsized>::Archived>;
    type Resolver = BoxResolver<<[NonZeroU64] as ArchiveUnsized>::MetadataResolver>;

    unsafe fn resolve_with(
        field: &Vec<Id<T>>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let slice = ids_to_nonzeros(field.as_slice());
        ArchivedBox::resolve_from_ref(slice, pos, resolver, out);
    }
}

impl<T, S: Serializer + Fallible> SerializeWith<Vec<Id<T>>, S> for IdVec {
    fn serialize_with(
        field: &Vec<Id<T>>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let slice = ids_to_nonzeros(field.as_slice());

        unsafe { ArchivedBox::serialize_copy_from_slice(slice, serializer) }
    }
}

fn ids_to_nonzeros<T>(ids: &[Id<T>]) -> &[NonZeroU64] {
    // SAFETY: Id<T> essentially only consists of a NonZeroU64
    unsafe { mem::transmute(ids) }
}
