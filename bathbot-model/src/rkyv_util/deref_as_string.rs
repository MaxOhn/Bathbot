use std::ops::Deref;

use rkyv::{
    ser::Serializer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Fallible,
};
use rosu_v2::prelude::{CountryCode, Username};

pub struct DerefAsString;

impl<T> ArchiveWith<T> for DerefAsString
where
    T: Deref<Target = str>,
{
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    unsafe fn resolve_with(
        field: &T,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        ArchivedString::resolve_from_str(field, pos, resolver, out);
    }
}

impl<S, T> SerializeWith<T, S> for DerefAsString
where
    S: Serializer + Fallible + ?Sized,
    T: Deref<Target = str>,
{
    #[inline]
    fn serialize_with(
        field: &T,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as rkyv::Fallible>::Error> {
        ArchivedString::serialize_from_str(field, serializer)
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedString, CountryCode, D> for DerefAsString {
    #[inline]
    fn deserialize_with(
        field: &ArchivedString,
        _: &mut D,
    ) -> Result<CountryCode, <D as Fallible>::Error> {
        Ok(field.as_str().into())
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedString, Username, D> for DerefAsString {
    #[inline]
    fn deserialize_with(
        field: &ArchivedString,
        _: &mut D,
    ) -> Result<Username, <D as Fallible>::Error> {
        Ok(field.as_str().into())
    }
}
