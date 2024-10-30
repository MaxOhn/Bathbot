use rkyv::{
    rancor::Fallible,
    ser::{Allocator, Writer},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, SerializeWith},
    Archive, Place, Serialize,
};

pub struct SliceAsVec;

impl<F: Archive> ArchiveWith<[F]> for SliceAsVec {
    type Archived = ArchivedVec<F::Archived>;
    type Resolver = VecResolver;

    fn resolve_with(field: &[F], resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(field.len(), resolver, out);
    }
}

impl<F, S> SerializeWith<[F], S> for SliceAsVec
where
    F: Serialize<S>,
    S: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize_with(field: &[F], serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_slice(field, serializer)
    }
}
