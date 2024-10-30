use std::{marker::PhantomData, ops::Deref};

use eyre::{Report, Result};
use rkyv::{
    bytecheck::CheckBytes,
    rancor::{BoxedError, Strategy},
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    Deserialize, Portable,
};

pub struct CachedArchive<T> {
    bytes: AlignedVec<8>,
    phantom: PhantomData<T>,
}

impl<T> CachedArchive<T>
where
    T: Portable + for<'a> CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, BoxedError>>,
{
    pub fn new(bytes: AlignedVec<8>) -> Result<Self> {
        debug_assert!(align_of::<T>() <= 8);

        let slice = bytes.as_slice();
        let mut validator = Validator::new(ArchiveValidator::new(slice), ());
        rkyv::api::access_with_context::<T, _, _>(slice, &mut validator).map_err(Report::new)?;

        Ok(Self::new_unchecked(bytes))
    }
}

impl<T> CachedArchive<T> {
    const fn new_unchecked(bytes: AlignedVec<8>) -> Self {
        Self {
            bytes,
            phantom: PhantomData,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl<A: Portable> CachedArchive<A> {
    pub fn deserialize<T>(&self) -> Result<T>
    where
        A: Deserialize<T, Strategy<(), BoxedError>>,
    {
        rkyv::api::deserialize_using(self.deref(), Strategy::<_, BoxedError>::wrap(&mut ()))
            .map_err(Report::new)
    }
}

impl<T: Portable> Deref for CachedArchive<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { rkyv::access_unchecked::<T>(self.bytes.as_slice()) }
    }
}
