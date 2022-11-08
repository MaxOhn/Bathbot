use rkyv::{
    ser::Serializer,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Fallible,
};
use rosu_v2::prelude::CountryCode;

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
