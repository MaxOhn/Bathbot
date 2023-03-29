use std::ops::Deref;

use rkyv::{
    boxed::ArchivedBox,
    with::{ArchiveWith, SerializeWith},
    ArchiveUnsized, Archived, Fallible, Resolver, SerializeUnsized,
};

pub struct DerefAsBox;

impl<U, V> ArchiveWith<U> for DerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + ?Sized,
{
    type Archived = Archived<Box<V>>;
    type Resolver = Resolver<Box<V>>;

    #[inline]
    unsafe fn resolve_with(
        field: &U,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let deref: &V = field;
        ArchivedBox::resolve_from_ref(deref, pos, resolver, out);
    }
}

impl<S, U, V> SerializeWith<U, S> for DerefAsBox
where
    U: Deref<Target = V>,
    V: ArchiveUnsized + SerializeUnsized<S> + ?Sized,
    S: Fallible + ?Sized,
{
    #[inline]
    fn serialize_with(
        value: &U,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let deref: &V = value;

        ArchivedBox::serialize_from_ref(deref, serializer)
    }
}
