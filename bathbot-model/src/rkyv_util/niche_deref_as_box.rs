use std::ops::Deref;

use rkyv::{
    ArchiveUnsized, ArchivedMetadata, Place, SerializeUnsized,
    niche::option_box::{ArchivedOptionBox, OptionBoxResolver},
    rancor::Fallible,
    ser::Writer,
    with::{ArchiveWith, SerializeWith},
};

pub struct NicheDerefAsBox;

impl<U, V> ArchiveWith<Option<U>> for NicheDerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + ?Sized,
    ArchivedMetadata<V>: Default,
{
    type Archived = ArchivedOptionBox<V::Archived>;
    type Resolver = OptionBoxResolver;

    #[inline]
    fn resolve_with(field: &Option<U>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let deref: Option<&V> = field.as_deref();
        ArchivedOptionBox::resolve_from_option(deref, resolver, out);
    }
}

impl<S, U, V> SerializeWith<Option<U>, S> for NicheDerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + SerializeUnsized<S> + ?Sized,
    S: Writer + Fallible + ?Sized,
    ArchivedMetadata<V>: Default,
{
    #[inline]
    fn serialize_with(
        field: &Option<U>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let deref: Option<&V> = field.as_deref();

        ArchivedOptionBox::serialize_from_option(deref, serializer)
    }
}
