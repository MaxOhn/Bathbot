use std::marker::PhantomData;

use rkyv::{
    Place,
    rancor::Fallible,
    ser::{Allocator, Writer},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, SerializeWith, With},
};

/// Basically [`rkyv::with::Map`] but for `Box<[T]>`
pub struct MapBoxedSlice<T>(PhantomData<T>);

impl<A, O> ArchiveWith<Box<[O]>> for MapBoxedSlice<A>
where
    A: ArchiveWith<O>,
{
    type Archived = ArchivedVec<<A as ArchiveWith<O>>::Archived>;
    type Resolver = VecResolver;

    #[inline]
    fn resolve_with(field: &Box<[O]>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(field.len(), resolver, out);
    }
}

impl<S, A, O> SerializeWith<Box<[O]>, S> for MapBoxedSlice<A>
where
    S: Fallible + Allocator + Writer + ?Sized,
    A: ArchiveWith<O> + SerializeWith<O, S>,
{
    #[inline]
    fn serialize_with(field: &Box<[O]>, s: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_iter::<With<O, A>, _, _>(field.iter().map(With::cast), s)
    }
}
