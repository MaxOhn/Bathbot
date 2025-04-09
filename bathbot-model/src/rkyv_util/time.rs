use rkyv::{
    Archive, Archived, Place, Portable,
    bytecheck::CheckBytes,
    munge::munge,
    niche::niching::Niching,
    rancor::{Fallible, Source},
    rend::u64_le,
    traits::NoUndef,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};
use time::{Date, OffsetDateTime, error::ComponentRange};

pub struct DateTimeRkyv;

#[derive(Copy, Clone, CheckBytes, Portable, PartialEq, Eq)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ArchivedDateTime {
    // First 64 bits
    a: u64_le,
    // Last 64 bits
    b: u64_le,
}

unsafe impl NoUndef for ArchivedDateTime {}

impl ArchivedDateTime {
    pub const fn new(datetime: OffsetDateTime) -> Self {
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

impl Niching<ArchivedDateTime> for DateTimeRkyv {
    unsafe fn is_niched(niched: *const ArchivedDateTime) -> bool {
        unsafe { *niched == ArchivedDateTime::new(OffsetDateTime::UNIX_EPOCH) }
    }

    fn resolve_niched(out: Place<ArchivedDateTime>) {
        Self::resolve_with(&OffsetDateTime::UNIX_EPOCH, (), out);
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

impl<S: Fallible + ?Sized> SerializeWith<Date, S> for DateRkyv {
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

/// Niching for the unix epoch, i.e. 00:00:00 UTC on 1 January 1970
pub struct UnixEpoch;

impl UnixEpoch {
    const NICHED: ArchivedDateTime = ArchivedDateTime::new(OffsetDateTime::UNIX_EPOCH);
}

impl Niching<ArchivedDateTime> for UnixEpoch {
    unsafe fn is_niched(niched: *const ArchivedDateTime) -> bool {
        unsafe { *niched == Self::NICHED }
    }

    fn resolve_niched(out: Place<ArchivedDateTime>) {
        out.write(Self::NICHED);
    }
}
