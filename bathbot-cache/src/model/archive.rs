use std::{marker::PhantomData, ops::Deref};

use rkyv::{
    bytecheck::CheckBytes,
    rancor::{Panic, ResultExt, Strategy},
    seal::Seal,
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    with::{ArchiveWith, DeserializeWith, With},
    Archive, Deserialize,
};

type CacheValidator<'a> = Validator<ArchiveValidator<'a>, ()>;
pub(crate) type ValidatorStrategy<'a> = Strategy<CacheValidator<'a>, Panic>;

#[derive(Clone)]
pub struct CachedArchive<T: ?Sized> {
    bytes: AlignedVec<8>,
    phantom: PhantomData<T>,
}

fn validator(bytes: &[u8]) -> CacheValidator<'_> {
    Validator::new(ArchiveValidator::new(bytes), ())
}

impl<T: ?Sized> CachedArchive<T> {
    pub(crate) fn new(bytes: AlignedVec<8>) -> Self
    where
        T: for<'a> Archive<Archived: CheckBytes<ValidatorStrategy<'a>>>,
    {
        debug_assert!(align_of::<T::Archived>() <= 8);

        let slice = bytes.as_slice();
        let mut validator = validator(slice);
        rkyv::api::access_with_context::<T::Archived, _, _>(slice, &mut validator).always_ok();

        unsafe { Self::new_unchecked(bytes) }
    }

    pub(crate) fn new_with<W>(bytes: AlignedVec<8>) -> Self
    where
        W: for<'a> ArchiveWith<T, Archived: CheckBytes<ValidatorStrategy<'a>>>,
    {
        type ArchivedWith<T, W> = <With<T, W> as Archive>::Archived;

        debug_assert!(align_of::<ArchivedWith<T, W>>() <= 8);

        let slice = bytes.as_slice();
        let mut validator = validator(slice);
        rkyv::api::access_with_context::<ArchivedWith<T, W>, _, _>(slice, &mut validator)
            .always_ok();

        unsafe { Self::new_unchecked(bytes) }
    }

    unsafe fn new_unchecked(bytes: AlignedVec<8>) -> Self {
        Self {
            bytes,
            phantom: PhantomData,
        }
    }

    pub fn into_bytes(self) -> AlignedVec<8> {
        self.bytes
    }
}

impl<T: Archive + ?Sized> Deref for CachedArchive<T> {
    type Target = T::Archived;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Bytes were checked upon creation
        unsafe { rkyv::access_unchecked::<T::Archived>(&self.bytes) }
    }
}

impl<T: ?Sized> CachedArchive<T>
where
    T: for<'a> Archive<Archived: CheckBytes<ValidatorStrategy<'a>>>,
{
    pub fn deserialize_into<O>(&self) -> O
    where
        T::Archived: Deserialize<O, Strategy<(), Panic>>,
    {
        rkyv::api::deserialize_using(self.deref(), Strategy::<_, Panic>::wrap(&mut ())).always_ok()
    }

    pub fn mutate<F>(&mut self, f: F)
    where
        F: FnOnce(Seal<'_, T::Archived>),
    {
        let mut context = validator(&self.bytes);
        let pos = rkyv::api::root_position::<T::Archived>(self.bytes.len());
        rkyv::api::check_pos_with_context::<T::Archived, _, Panic>(&self.bytes, pos, &mut context)
            .always_ok();
        let archived =
            unsafe { rkyv::api::access_pos_unchecked_mut::<T::Archived>(&mut self.bytes, pos) };

        f(archived);
    }
}

impl<T: ?Sized> CachedArchive<T> {
    pub fn deref_with<W: ArchiveWith<T>>(&self) -> &<W as ArchiveWith<T>>::Archived {
        // SAFETY: Bytes were checked upon creation
        unsafe { rkyv::access_unchecked::<<W as ArchiveWith<T>>::Archived>(&self.bytes) }
    }

    pub fn deserialize_into_with<O, W>(&self) -> O
    where
        W: ArchiveWith<T>
            + DeserializeWith<<W as ArchiveWith<T>>::Archived, O, Strategy<(), Panic>>,
    {
        let archived =
            unsafe { rkyv::access_unchecked::<<W as ArchiveWith<T>>::Archived>(&self.bytes) };

        W::deserialize_with(archived, Strategy::wrap(&mut ())).always_ok()
    }
}

impl<T> CachedArchive<T> {
    pub fn deserialize_with<W>(&self) -> T
    where
        W: ArchiveWith<T>
            + DeserializeWith<<W as ArchiveWith<T>>::Archived, T, Strategy<(), Panic>>,
    {
        let archived =
            unsafe { rkyv::access_unchecked::<<W as ArchiveWith<T>>::Archived>(&self.bytes) };

        W::deserialize_with(archived, Strategy::wrap(&mut ())).always_ok()
    }
}
