use bb8_redis::redis::{
    ErrorKind, FromRedisValue, RedisError, RedisResult, RedisWrite, ToRedisArgs, Value,
};
use rkyv::util::AlignedVec;

pub(crate) struct AlignedVecRedisArgs(pub(crate) AlignedVec<8>);

impl ToRedisArgs for AlignedVecRedisArgs {
    #[inline]
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.0.as_slice().write_redis_args(out)
    }
}

impl FromRedisValue for AlignedVecRedisArgs {
    #[inline]
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            Value::Data(data) => {
                let mut bytes = AlignedVec::new();
                bytes.reserve_exact(data.len());
                bytes.extend_from_slice(data);

                Ok(Self(bytes))
            }
            _ => Err(RedisError::from((
                ErrorKind::TypeError,
                "Response was of incompatible type",
                "Response type not byte list compatible".to_owned(),
            ))),
        }
    }
}
