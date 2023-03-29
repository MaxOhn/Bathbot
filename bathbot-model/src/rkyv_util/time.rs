use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Fallible,
};
use time::{Date, OffsetDateTime};

pub struct DateTimeRkyv;

impl ArchiveWith<OffsetDateTime> for DateTimeRkyv {
    type Archived = i128;
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

impl<S: Fallible> SerializeWith<OffsetDateTime, S> for DateTimeRkyv {
    #[inline]
    fn serialize_with(_: &OffsetDateTime, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible> DeserializeWith<i128, OffsetDateTime, D> for DateTimeRkyv {
    #[inline]
    fn deserialize_with(field: &i128, _: &mut D) -> Result<OffsetDateTime, D::Error> {
        Ok(OffsetDateTime::from_unix_timestamp_nanos(*field).unwrap())
    }
}

pub struct DateRkyv;

impl ArchiveWith<Date> for DateRkyv {
    type Archived = i32;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &Date,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let year = field.year();
        let ordinal = field.ordinal();
        let value = (year << 9) | ordinal as i32;
        value.resolve(pos, resolver, out);
    }
}

impl<S: Fallible> SerializeWith<Date, S> for DateRkyv {
    #[inline]
    fn serialize_with(_: &Date, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible> DeserializeWith<i32, Date, D> for DateRkyv {
    #[inline]
    fn deserialize_with(field: &i32, _: &mut D) -> Result<Date, D::Error> {
        let year = *field >> 9;
        let ordinal = (*field & 0x1FF) as u16;

        Ok(Date::from_ordinal_date(year, ordinal).unwrap())
    }
}
