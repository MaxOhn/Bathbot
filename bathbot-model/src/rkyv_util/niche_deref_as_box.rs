use std::ops::Deref;

use rkyv::{
    niche::option_box::{ArchivedOptionBox, OptionBoxResolver},
    ser::Serializer,
    with::{ArchiveWith, SerializeWith},
    ArchiveUnsized, ArchivedMetadata, Fallible, SerializeUnsized,
};

pub struct NicheDerefAsBox;

impl<U, V> ArchiveWith<Option<U>> for NicheDerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + ?Sized,
    ArchivedMetadata<V>: Default,
{
    type Archived = ArchivedOptionBox<V::Archived>;
    type Resolver = OptionBoxResolver<V::MetadataResolver>;

    #[inline]
    unsafe fn resolve_with(
        field: &Option<U>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let deref: Option<&V> = field.as_deref();
        ArchivedOptionBox::resolve_from_option(deref, pos, resolver, out);
    }
}

impl<S, U, V> SerializeWith<Option<U>, S> for NicheDerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + SerializeUnsized<S> + ?Sized,
    S: Serializer + Fallible + ?Sized,
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
