use rkyv::{
    bytecheck::CheckBytes,
    munge::munge,
    rancor::{Fallible, Source},
    rend::u64_le,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Place, Portable,
};
use time::{error::ComponentRange, Date, OffsetDateTime};

pub struct DateTimeRkyv;

#[derive(Copy, Clone, CheckBytes, Portable)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ArchivedDateTime {
    // First 64 bits
    a: u64_le,
    // Last 64 bits
    b: u64_le,
}

impl ArchivedDateTime {
    pub fn new(datetime: OffsetDateTime) -> Self {
        let unix_timestamp_nanos = datetime.unix_timestamp_nanos();

        Self {
            a: u64_le::from_native((unix_timestamp_nanos >> 64) as u64),
            b: u64_le::from_native(unix_timestamp_nanos as u64),
        }
    }

    pub fn try_deserialize<E: Source>(&self) -> Result<OffsetDateTime, E> {
        let unix_timestamp_nanos =
            ((self.a.to_native() as i128) << 64) | self.b.to_native() as i128;

        OffsetDateTime::from_unix_timestamp_nanos(unix_timestamp_nanos).map_err(Source::new)
    }
}

impl ArchiveWith<OffsetDateTime> for DateTimeRkyv {
    type Archived = ArchivedDateTime;
    type Resolver = ();

    #[inline]
    fn resolve_with(field: &OffsetDateTime, _: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedDateTime { a, b } = out);
        let archived = ArchivedDateTime::new(*field);
        archived.a.resolve((), a);
        archived.b.resolve((), b);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<OffsetDateTime, S> for DateTimeRkyv {
    #[inline]
    fn serialize_with(_: &OffsetDateTime, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible<Error: Source>> DeserializeWith<ArchivedDateTime, OffsetDateTime, D>
    for DateTimeRkyv
{
    #[inline]
    fn deserialize_with(
        archived: &ArchivedDateTime,
        _: &mut D,
    ) -> Result<OffsetDateTime, D::Error> {
        archived.try_deserialize()
    }
}

pub struct DateRkyv;

impl DateRkyv {
    pub fn try_deserialize(archived: Archived<i32>) -> Result<Date, ComponentRange> {
        let year = archived >> 9;
        let ordinal = (archived & 0x1FF) as u16;

        Date::from_ordinal_date(year, ordinal)
    }
}

impl ArchiveWith<Date> for DateRkyv {
    type Archived = Archived<i32>;
    type Resolver = ();

    #[inline]
    fn resolve_with(field: &Date, resolver: Self::Resolver, out: Place<Self::Archived>) {
        let year = field.year();
        let ordinal = field.ordinal();
        let value = (year << 9) | ordinal as i32;
        value.resolve(resolver, out);
    }
}

impl<S: Fallible> SerializeWith<Date, S> for DateRkyv {
    #[inline]
    fn serialize_with(_: &Date, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible<Error: Source>> DeserializeWith<Archived<i32>, Date, D> for DateRkyv {
    #[inline]
    fn deserialize_with(field: &Archived<i32>, _: &mut D) -> Result<Date, D::Error> {
        Self::try_deserialize(*field).map_err(Source::new)
    }
}
