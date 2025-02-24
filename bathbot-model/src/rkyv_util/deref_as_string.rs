use std::ops::Deref;

use rkyv::{
    Place,
    rancor::{Fallible, Source},
    ser::Writer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};

pub struct DerefAsString;

impl<T> ArchiveWith<T> for DerefAsString
where
    T: Deref<Target = str>,
{
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    fn resolve_with(field: &T, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(field, resolver, out);
    }
}

impl<S, T> SerializeWith<T, S> for DerefAsString
where
    S: Writer + Fallible<Error: Source> + ?Sized,
    T: Deref<Target = str>,
{
    #[inline]
    fn serialize_with(field: &T, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field, serializer)
    }
}

impl<T, D> DeserializeWith<ArchivedString, T, D> for DerefAsString
where
    T: for<'s> From<&'s str>,
    D: Fallible + ?Sized,
{
    #[inline]
    fn deserialize_with(field: &ArchivedString, _: &mut D) -> Result<T, D::Error> {
        Ok(T::from(field.as_str()))
    }
}
