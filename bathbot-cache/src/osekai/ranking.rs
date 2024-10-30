use std::{marker::PhantomData, time::Duration};

use bathbot_model::{rkyv_util::SliceAsVec, OsekaiRanking};
use rkyv::{
    bytecheck::CheckBytes, rancor::BoxedError, util::AlignedVec, vec::ArchivedVec, Archived,
    Serialize,
};

use crate::data::{
    serialize_using_arena_and_with, BathbotRedisData, BathbotRedisSerializer, BathbotRedisValidator,
};

pub struct CacheOsekaiRanking<R>(PhantomData<R>);

impl<R> BathbotRedisData for CacheOsekaiRanking<R>
where
    R: OsekaiRanking,
    R::Entry: for<'a> Serialize<
        BathbotRedisSerializer<'a>,
        Archived: CheckBytes<BathbotRedisValidator<'a>>,
    >,
{
    type Archived = ArchivedVec<Archived<R::Entry>>;
    type Original = [R::Entry];
    type With = SliceAsVec;

    const EXPIRE: Option<Duration> = Some(Duration::from_secs(7200));

    fn serialize(data: &Self::Original) -> Result<AlignedVec<8>, BoxedError> {
        serialize_using_arena_and_with::<_, Self::With>(data)
    }
}
