use bb8_redis::redis::{RedisWrite, ToRedisArgs};
use rkyv::AlignedVec;

pub(crate) struct AlignedVecRedisArgs(pub(crate) AlignedVec);

impl ToRedisArgs for AlignedVecRedisArgs {
    #[inline]
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.0.as_slice().write_redis_args(out)
    }
}
