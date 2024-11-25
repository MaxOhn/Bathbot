use rkyv::{
    rancor::{Fallible, Source},
    ser::Writer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, SerializeWith},
    Place, SerializeUnsized,
};

pub struct StrAsString;

impl ArchiveWith<&str> for StrAsString {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    fn resolve_with(field: &&str, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(field, resolver, out);
    }
}

impl<S> SerializeWith<&str, S> for StrAsString
where
    str: SerializeUnsized<S>,
    S: Fallible<Error: Source> + Writer + ?Sized,
{
    fn serialize_with(field: &&str, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field, serializer)
    }
}
