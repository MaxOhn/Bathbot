use std::{marker::PhantomData, ops::Deref};

use bb8_redis::redis::{ErrorKind, FromRedisValue, RedisError, RedisResult, Value};
use rkyv::{
    with::{ArchiveWith, DeserializeWith, With},
    Archive, Archived, Deserialize, Infallible,
};

#[derive(Clone)]
pub struct CachedArchive<T> {
    bytes: Vec<u8>,
    phantom: PhantomData<T>,
}

impl<T> CachedArchive<T> {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
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

impl<T> CachedArchive<T>
where
    T: Archive,
    Archived<T>: Deserialize<T, Infallible>,
{
    pub fn deserialize(&self) -> T {
        <Archived<T> as Deserialize<T, _>>::deserialize(self, &mut Infallible).unwrap()
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
            Value::Data(bytes) => Ok(Self::new(bytes.to_owned())),
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
