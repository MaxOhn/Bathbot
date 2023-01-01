use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Fallible,
};
use time::OffsetDateTime;

pub struct DateTimeWrapper;

impl ArchiveWith<OffsetDateTime> for DateTimeWrapper {
    type Archived = Archived<i128>;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &OffsetDateTime,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        Archive::resolve(&field.unix_timestamp_nanos(), pos, resolver, out);
    }
}

impl<D: Fallible> DeserializeWith<i128, OffsetDateTime, D> for DateTimeWrapper {
    #[inline]
    fn deserialize_with(field: &Archived<i128>, _: &mut D) -> Result<OffsetDateTime, D::Error> {
        Ok(OffsetDateTime::from_unix_timestamp_nanos(*field).unwrap())
    }
}

impl<S: Fallible> SerializeWith<OffsetDateTime, S> for DateTimeWrapper {
    #[inline]
    fn serialize_with(_: &OffsetDateTime, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}
