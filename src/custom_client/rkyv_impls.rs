use chrono::{DateTime, TimeZone, Utc};
use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Fallible,
};

pub struct DateTimeWrapper;

impl ArchiveWith<DateTime<Utc>> for DateTimeWrapper {
    type Archived = Archived<i64>;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &DateTime<Utc>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        Archive::resolve(&field.timestamp_millis(), pos, resolver, out);
    }
}

impl<D: Fallible> DeserializeWith<i64, DateTime<Utc>, D> for DateTimeWrapper {
    #[inline]
    fn deserialize_with(field: &Archived<i64>, _: &mut D) -> Result<DateTime<Utc>, D::Error> {
        Ok(Utc.timestamp_millis(*field))
    }
}

impl<S: Fallible> SerializeWith<DateTime<Utc>, S> for DateTimeWrapper {
    #[inline]
    fn serialize_with(_: &DateTime<Utc>, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}
