use std::marker::PhantomData;

use rkyv::{
    Place,
    rancor::Fallible,
    ser::{Allocator, Writer},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, DeserializeWith, Map, SerializeWith},
};

pub struct MapUnwrapOrDefault<T>(PhantomData<T>);

impl<A, O> ArchiveWith<Option<Vec<O>>> for MapUnwrapOrDefault<A>
where
    A: ArchiveWith<O>,
{
    type Archived = ArchivedVec<<A as ArchiveWith<O>>::Archived>;
    type Resolver = VecResolver;

    #[inline]
    fn resolve_with(field: &Option<Vec<O>>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let len = field.as_ref().map_or(0, Vec::len);
        ArchivedVec::resolve_from_len(len, resolver, out);
    }
}

impl<A, O, S> SerializeWith<Option<Vec<O>>, S> for MapUnwrapOrDefault<A>
where
    A: SerializeWith<O, S>,
    S: Fallible + Allocator + Writer + ?Sized,
{
    #[inline]
    fn serialize_with(
        field: &Option<Vec<O>>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        match field {
            Some(vec) => Map::<A>::serialize_with(vec, serializer),
            None => Ok(VecResolver::from_pos(serializer.pos())),
        }
    }
}

impl<A, O, D> DeserializeWith<ArchivedVec<<A as ArchiveWith<O>>::Archived>, Vec<O>, D>
    for MapUnwrapOrDefault<A>
where
    A: ArchiveWith<O> + DeserializeWith<<A as ArchiveWith<O>>::Archived, O, D>,
    D: Fallible + ?Sized,
{
    #[inline]
    fn deserialize_with(
        field: &ArchivedVec<<A as ArchiveWith<O>>::Archived>,
        deserializer: &mut D,
    ) -> Result<Vec<O>, D::Error> {
        Map::<A>::deserialize_with(field, deserializer)
    }
}
