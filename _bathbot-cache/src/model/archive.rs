use std::{marker::PhantomData, ops::Deref, pin::Pin};

use bb8_redis::redis::{ErrorKind, FromRedisValue, RedisError, RedisResult, Value};
use rkyv::{
    with::{ArchiveWith, DeserializeWith, With},
    AlignedVec, Archive, Archived, Deserialize, Infallible,
};

#[derive(Clone)]
pub struct CachedArchive<T> {
    bytes: AlignedVec,
    phantom: PhantomData<T>,
}

impl<T> CachedArchive<T> {
    pub(crate) fn new(bytes: AlignedVec) -> Self {
        Self {
            bytes,
            phantom: PhantomData,
        }
    }
}

impl<T: Archive> Deref for CachedArchive<T> {
    type Target = Archived<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Bytes originate from redis which only stores valid archived data
        unsafe { rkyv::archived_root::<T>(&self.bytes) }
    }
}

impl<T: Archive> CachedArchive<T> {
    pub fn deserialize<O>(&self) -> O
    where
        Archived<T>: Deserialize<O, Infallible>,
    {
        <Archived<T> as Deserialize<O, _>>::deserialize(self, &mut Infallible).unwrap()
    }

    pub fn mutate<F>(&mut self, f: F)
    where
        F: FnOnce(Pin<&mut <T as Archive>::Archived>),
    {
        // SAFETY: Bytes originate from redis which only stores valid archived data
        let archived = unsafe { rkyv::archived_root_mut::<T>(Pin::new(&mut self.bytes)) };
        f(archived);
    }
}

impl<T> CachedArchive<T> {
    pub fn deserialize_with<W>(&self) -> T
    where
        W: ArchiveWith<T> + DeserializeWith<<W as ArchiveWith<T>>::Archived, T, Infallible>,
    {
        // SAFETY: Bytes originate from redis which only stores valid archived data
        let archived = unsafe { rkyv::archived_root::<With<_, W>>(&self.bytes) };

        W::deserialize_with(archived, &mut Infallible).unwrap()
    }
}

impl<T> FromRedisValue for CachedArchive<T> {
    #[inline]
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            Value::Data(data) => {
                let mut bytes = AlignedVec::new();
                bytes.reserve_exact(data.len());
                bytes.extend_from_slice(data);

                Ok(Self::new(bytes))
            }
            _ => Err(RedisError::from((
                ErrorKind::TypeError,
                "Response was of incompatible type",
                format!(
                    "Response type not byte list compatible. (response was {:?})",
                    std::any::type_name::<T>()
                ),
            ))),
        }
    }
}
