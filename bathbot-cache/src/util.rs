use bb8_redis::redis::{ErrorKind, FromRedisValue, RedisError, RedisResult, Value};
use rkyv::util::AlignedVec;

pub(crate) struct BytesWrap<B>(pub(crate) B);

impl<const A: usize> FromRedisValue for BytesWrap<AlignedVec<A>> {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            Value::Data(data) => {
                let mut bytes = AlignedVec::new();
                bytes.reserve_exact(data.len());
                bytes.extend_from_slice(data);

                Ok(Self(bytes))
            }
            value => Err(RedisError::from((
                ErrorKind::TypeError,
                "Response was of incompatible type",
                format!("Response type not byte list compatible. (response was {value:?})"),
            ))),
        }
    }
}
