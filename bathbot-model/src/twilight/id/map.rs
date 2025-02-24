use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    marker::PhantomData,
    mem,
    num::NonZeroU64,
    ptr, slice,
};

use rkyv::{
    Archive, Archived, Deserialize, Place, Portable,
    bytecheck::CheckBytes,
    niche::option_nonzero::ArchivedOptionNonZeroU64,
    rancor::Fallible,
    ser::{Allocator, Writer, WriterExt},
    traits::NoUndef,
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, DeserializeWith, Map, SerializeWith, With},
};
use twilight_model::id::Id;

use super::{ArchivedId, IdRkyv};

/// Used to archive `Option<Id<T>>`, `Vec<Id<T>>`, `&[Id<T>]`,
/// and `Box<[Id<T>]>` more efficiently than [`Map<IdRkyv>`](rkyv::with::Map).
///
/// # Example
///
/// ```
/// # use rkyv::Archive;
/// use redlight::rkyv_util::id::IdRkyvMap;
/// use twilight_model::id::Id;
///
/// #[derive(Archive)]
/// struct Cached<'a, T> {
///     #[rkyv(with = IdRkyvMap)]
///     id_opt: Option<Id<T>>,
///     #[rkyv(with = IdRkyvMap)]
///     id_vec: Vec<Id<T>>,
///     #[rkyv(with = IdRkyvMap)]
///     id_slice: &'a [Id<T>],
///     #[rkyv(with = IdRkyvMap)]
///     id_box: Box<[Id<T>]>,
/// }
/// ```
pub struct IdRkyvMap;

#[derive(Portable, CheckBytes)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
/// An efficiently archived `Option<Id<T>>`.
pub struct ArchivedIdOption<T> {
    inner: ArchivedOptionNonZeroU64,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<T> Clone for ArchivedIdOption<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ArchivedIdOption<T> {}

impl<T> PartialEq for ArchivedIdOption<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> Eq for ArchivedIdOption<T> {}

impl<T> PartialEq<Option<Id<T>>> for ArchivedIdOption<T> {
    fn eq(&self, other: &Option<Id<T>>) -> bool {
        self.to_id_option() == *other
    }
}

unsafe impl<T> NoUndef for ArchivedIdOption<T> {}

impl<T> ArchivedIdOption<T> {
    /// Convert into an `Option<NonZeroU64>`.
    pub fn to_nonzero_option(mut self) -> Option<NonZeroU64> {
        self.inner.take().map(NonZeroU64::from)
    }

    /// Convert into an `Option<Id<T>>`.
    pub fn to_id_option(self) -> Option<Id<T>> {
        self.to_nonzero_option().map(Id::from)
    }

    /// Resolves an `ArchivedIdOption` from an `Option<Id<T>>`.
    #[allow(clippy::similar_names)]
    pub fn resolve_from_id(opt: Option<Id<T>>, out: Place<Self>) {
        rkyv::munge::munge!(let Self { inner, _phantom } = out);
        ArchivedOptionNonZeroU64::resolve_from_option(opt.map(Id::into_nonzero), inner);
    }
}

impl<T> Debug for ArchivedIdOption<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(&self.to_nonzero_option(), f)
    }
}

impl<T> ArchiveWith<Option<Id<T>>> for IdRkyvMap {
    type Archived = ArchivedIdOption<T>;
    type Resolver = ();

    fn resolve_with(id: &Option<Id<T>>, (): Self::Resolver, out: Place<Self::Archived>) {
        ArchivedIdOption::resolve_from_id(*id, out);
    }
}

impl<S: Fallible + ?Sized, T> SerializeWith<Option<Id<T>>, S> for IdRkyvMap {
    fn serialize_with(_: &Option<Id<T>>, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible + ?Sized, T> DeserializeWith<ArchivedIdOption<T>, Option<Id<T>>, D> for IdRkyvMap {
    fn deserialize_with(
        archived: &ArchivedIdOption<T>,
        deserializer: &mut D,
    ) -> Result<Option<Id<T>>, D::Error> {
        archived.deserialize(deserializer)
    }
}

impl<D: Fallible + ?Sized, T> Deserialize<Option<Id<T>>, D> for ArchivedIdOption<T> {
    fn deserialize(&self, _: &mut D) -> Result<Option<Id<T>>, <D as Fallible>::Error> {
        Ok(self.to_id_option())
    }
}

/// Auxiliary trait to provide the most efficient (de)serializations of
/// `&[Id<T>]` across every endian.
trait INonZeroU64: Archive<Archived = Self> {
    /// Serialize `&[Id<T>]` while leveraging when `Self == Archived<Self>`.
    fn serialize<S, T>(
        list: &[Id<T>],
        serializer: &mut S,
    ) -> Result<VecResolver, <S as Fallible>::Error>
    where
        S: Fallible + Allocator + Writer + ?Sized;

    /// Deserialize an archived `Vec<Id<T>>` while leveraging when `Self ==
    /// Archived<Self>`.
    fn deserialize<T, D>(
        archived: &ArchivedVec<ArchivedId<T>>,
        deserializer: &mut D,
    ) -> Result<Vec<Id<T>>, D::Error>
    where
        D: Fallible + ?Sized;
}

macro_rules! impl_non_zero {
    ($ty:path, $endian:literal) => {
        impl INonZeroU64 for $ty {
            fn serialize<S, T>(
                ids: &[Id<T>],
                serializer: &mut S,
            ) -> Result<VecResolver, <S as Fallible>::Error>
            where
                S: Fallible + Allocator + Writer + ?Sized,
            {
                const fn with_ids<T>(ids: &[Id<T>]) -> &[With<Id<T>, IdRkyv>] {
                    let ptr = ptr::from_ref(ids) as *const [With<Id<T>, IdRkyv>];

                    // SAFETY: `With` is just a transparent wrapper
                    unsafe { &*ptr }
                }

                if cfg!(target_endian = $endian) {
                    let pos =
                        serializer.align_for::<<With<Id<T>, IdRkyv> as Archive>::Archived>()?;

                    // # Safety: `NonZeroU64` and `Archived<NonZeroU64>` share
                    // the same layout.
                    let as_bytes = unsafe {
                        slice::from_raw_parts(ids.as_ptr().cast::<u8>(), mem::size_of_val(ids))
                    };

                    serializer.write(as_bytes)?;

                    Ok(VecResolver::from_pos(pos))
                } else {
                    ArchivedVec::serialize_from_slice(with_ids(ids), serializer)
                }
            }

            fn deserialize<T, D>(
                archived: &ArchivedVec<ArchivedId<T>>,
                deserializer: &mut D,
            ) -> Result<Vec<Id<T>>, D::Error>
            where
                D: Fallible + ?Sized,
            {
                if cfg!(target_endian = $endian) {
                    // # Safety: `NonZeroU64` and `Archived<NonZeroU64>` share
                    // the same layout.
                    let slice = unsafe { &*(ptr::from_ref(archived.as_slice()) as *const [Id<T>]) };

                    Ok(slice.to_owned())
                } else {
                    With::<_, Map<IdRkyv>>::cast(archived).deserialize(deserializer)
                }
            }
        }
    };
}

impl_non_zero!(rkyv::rend::NonZeroU64_le, "little");
impl_non_zero!(rkyv::rend::NonZeroU64_be, "big");

// Vec<Id<T>>

impl<T> ArchiveWith<Vec<Id<T>>> for IdRkyvMap {
    type Archived = ArchivedVec<ArchivedId<T>>;
    type Resolver = VecResolver;

    fn resolve_with(ids: &Vec<Id<T>>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(ids.len(), resolver, out);
    }
}

impl<S, T> SerializeWith<Vec<Id<T>>, S> for IdRkyvMap
where
    S: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize_with(
        ids: &Vec<Id<T>>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        <Archived<NonZeroU64> as INonZeroU64>::serialize(ids, serializer)
    }
}

impl<D: Fallible + ?Sized, T> DeserializeWith<ArchivedVec<ArchivedId<T>>, Vec<Id<T>>, D>
    for IdRkyvMap
{
    fn deserialize_with(
        archived: &ArchivedVec<ArchivedId<T>>,
        deserializer: &mut D,
    ) -> Result<Vec<Id<T>>, <D as Fallible>::Error> {
        <Archived<NonZeroU64> as INonZeroU64>::deserialize(archived, deserializer)
    }
}

// &[Id<T>]

impl<T> ArchiveWith<&[Id<T>]> for IdRkyvMap {
    type Archived = ArchivedVec<ArchivedId<T>>;
    type Resolver = VecResolver;

    fn resolve_with(ids: &&[Id<T>], resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(ids.len(), resolver, out);
    }
}

impl<S, T> SerializeWith<&[Id<T>], S> for IdRkyvMap
where
    S: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize_with(
        ids: &&[Id<T>],
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        <Archived<NonZeroU64> as INonZeroU64>::serialize(ids, serializer)
    }
}

// Box<[Id<T>]>

impl<T> ArchiveWith<Box<[Id<T>]>> for IdRkyvMap {
    type Archived = ArchivedVec<ArchivedId<T>>;
    type Resolver = VecResolver;

    fn resolve_with(ids: &Box<[Id<T>]>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(ids.len(), resolver, out);
    }
}

impl<S, T> SerializeWith<Box<[Id<T>]>, S> for IdRkyvMap
where
    S: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize_with(
        ids: &Box<[Id<T>]>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        <Archived<NonZeroU64> as INonZeroU64>::serialize(ids, serializer)
    }
}

impl<D: Fallible + ?Sized, T> DeserializeWith<ArchivedVec<ArchivedId<T>>, Box<[Id<T>]>, D>
    for IdRkyvMap
{
    fn deserialize_with(
        archived: &ArchivedVec<ArchivedId<T>>,
        deserializer: &mut D,
    ) -> Result<Box<[Id<T>]>, <D as Fallible>::Error> {
        <Archived<NonZeroU64> as INonZeroU64>::deserialize(archived, deserializer)
            .map(Vec::into_boxed_slice)
    }
}

#[cfg(test)]
mod tests {
    use rkyv::rancor::{Panic, Strategy};
    use twilight_model::id::marker::GuildMarker;

    use super::*;

    #[test]
    fn id_rkyv_map() {
        type GuildId = Id<GuildMarker>;
        type Wrap<'a> = With<&'a [GuildId], IdRkyvMap>;

        let ids = vec![GuildId::new(1), Id::new(2), Id::new(3)];
        let slice = ids.as_slice();
        let with = Wrap::cast(&slice);

        let bytes = rkyv::to_bytes::<Panic>(with).unwrap();
        let archived =
            rkyv::access::<<IdRkyvMap as ArchiveWith<&[GuildId]>>::Archived, Panic>(&bytes)
                .unwrap();
        let mut deserializer = ();
        let strategy = Strategy::<_, Panic>::wrap(&mut deserializer);
        let deserialized: Vec<GuildId> = IdRkyvMap::deserialize_with(archived, strategy).unwrap();

        assert_eq!(ids, deserialized);
    }
}
