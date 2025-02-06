use std::{marker::PhantomData, ops::Deref};

use eyre::Result;
use rkyv::{
    bytecheck::CheckBytes,
    rancor::{BoxedError, Strategy},
    seal::Seal,
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    with::DeserializeWith,
    Deserialize, Portable,
};

#[derive(Clone)]
pub struct CachedArchive<T> {
    bytes: AlignedVec<8>,
    phantom: PhantomData<T>,
}

pub type ValidatorStrategy<'a> = Strategy<Validator<ArchiveValidator<'a>, ()>, BoxedError>;
pub type DeserializerStrategy = Strategy<(), BoxedError>;

impl<T> CachedArchive<T>
where
    T: Portable + for<'a> CheckBytes<ValidatorStrategy<'a>>,
{
    pub fn new(bytes: AlignedVec<8>) -> Result<Self, BoxedError> {
        debug_assert!(align_of::<T>() <= 8);

        let slice = bytes.as_slice();
        let mut validator = Validator::new(ArchiveValidator::new(slice), ());
        rkyv::api::access_with_context::<T, _, _>(slice, &mut validator)?;

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

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl<T: Portable> Deref for CachedArchive<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { rkyv::access_unchecked::<T>(self.as_bytes()) }
    }
}

impl<T: Portable> CachedArchive<T> {
    pub fn mutate<F: FnOnce(Seal<'_, T>)>(&mut self, f: F) {
        f(unsafe { rkyv::api::access_unchecked_mut::<T>(&mut self.bytes) })
    }

    pub fn try_deserialize<U>(&self) -> Result<U, BoxedError>
    where
        T: Deserialize<U, DeserializerStrategy>,
    {
        self.deserialize(Strategy::wrap(&mut ()))
    }

    pub fn deserialize_with<W, U>(&self) -> Result<U, BoxedError>
    where
        W: DeserializeWith<T, U, DeserializerStrategy>,
    {
        W::deserialize_with(self, Strategy::wrap(&mut ()))
    }
}
