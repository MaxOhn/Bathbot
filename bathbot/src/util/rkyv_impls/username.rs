use rkyv::{
    ser::Serializer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Fallible,
};
use rosu_v2::prelude::Username;

pub struct UsernameWrapper;

impl ArchiveWith<Username> for UsernameWrapper {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    unsafe fn resolve_with(
        field: &Username,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        ArchivedString::resolve_from_str(field.as_str(), pos, resolver, out);
    }
}

impl<S: Fallible + Serializer> SerializeWith<Username, S> for UsernameWrapper {
    #[inline]
    fn serialize_with(field: &Username, s: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field.as_str(), s)
    }
}

impl<D: Fallible> DeserializeWith<ArchivedString, Username, D> for UsernameWrapper {
    #[inline]
    fn deserialize_with(field: &ArchivedString, _: &mut D) -> Result<Username, D::Error> {
        Ok(Username::from_str(field.as_str()))
    }
}
