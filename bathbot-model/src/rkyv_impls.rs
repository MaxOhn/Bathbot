use rkyv::{
    ser::Serializer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Fallible,
};
use rosu_v2::prelude::{CountryCode, Username};
use time::{Date, OffsetDateTime};

pub struct UsernameWrapper;

impl ArchiveWith<Username> for UsernameWrapper {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    unsafe fn resolve_with(
        field: &Username,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        ArchivedString::resolve_from_str(field.as_str(), pos, resolver, out);
    }
}

impl<S: Fallible + Serializer> SerializeWith<Username, S> for UsernameWrapper {
    #[inline]
    fn serialize_with(field: &Username, s: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field.as_str(), s)
    }
}

impl<D: Fallible> DeserializeWith<ArchivedString, Username, D> for UsernameWrapper {
    #[inline]
    fn deserialize_with(field: &ArchivedString, _: &mut D) -> Result<Username, D::Error> {
        Ok(Username::from_str(field.as_str()))
    }
}

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

pub struct DateWrapper;

pub struct ArchivedDateUtc {
    value: Archived<i32>,
}

impl ArchiveWith<Date> for DateWrapper {
    type Archived = ArchivedDateUtc;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(field: &Date, pos: usize, _: Self::Resolver, out: *mut Self::Archived) {
        let (fp, fo) = {
            let fo = (&mut (*out).value) as *mut i32;
            (fo.cast::<u8>().offset_from(out.cast::<u8>()) as usize, fo)
        };

        let year = field.year();
        let ordinal = field.ordinal();
        let value = (year << 9) | ordinal as i32;

        #[allow(clippy::unit_arg)]
        value.resolve(pos + fp, (), fo);
    }
}

impl<S: Fallible> SerializeWith<Date, S> for DateWrapper {
    #[inline]
    fn serialize_with(_: &Date, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible> DeserializeWith<ArchivedDateUtc, Date, D> for DateWrapper {
    #[inline]
    fn deserialize_with(field: &ArchivedDateUtc, _: &mut D) -> Result<Date, D::Error> {
        let year = field.value >> 9;
        let ordinal = (field.value & 0x1FF) as _;

        Ok(Date::from_ordinal_date(year, ordinal).unwrap())
    }
}

pub struct CountryCodeWrapper;

#[derive(Copy, Clone)]
pub struct ArchivedCountryCode {
    inner: [u8; 2],
}

impl ArchivedCountryCode {
    pub fn new(country_code: [u8; 2]) -> Self {
        Self {
            inner: country_code,
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.inner)
            .unwrap_or_else(|_| panic!("country code bytes {:?} are invalid UTF-8", self.inner))
    }
}

impl ArchiveWith<CountryCode> for CountryCodeWrapper {
    type Archived = ArchivedCountryCode;
    type Resolver = [(); 2];

    #[inline]
    unsafe fn resolve_with(
        field: &CountryCode,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let out = out.cast();

        match field.as_bytes().try_into() {
            Ok(borrowed_array) => <[u8; 2] as Archive>::resolve(borrowed_array, pos, resolver, out),
            Err(_) => <[u8; 2] as Archive>::resolve(&[b'?', b'?'], pos, resolver, out),
        }
    }
}

impl<S: Fallible + Serializer> SerializeWith<CountryCode, S> for CountryCodeWrapper {
    #[inline]
    fn serialize_with(_: &CountryCode, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok([(); 2])
    }
}

impl<D: Fallible> DeserializeWith<ArchivedCountryCode, CountryCode, D> for CountryCodeWrapper {
    #[inline]
    fn deserialize_with(field: &ArchivedCountryCode, _: &mut D) -> Result<CountryCode, D::Error> {
        Ok(CountryCode::from_buf(field.inner).unwrap())
    }
}
