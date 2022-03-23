use chrono::{Date, Datelike, TimeZone, Utc};
use rkyv::{
    ser::Serializer,
    string::{ArchivedString, StringResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Fallible,
};
use rosu_v2::prelude::Username;

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

pub struct DateWrapper;

pub struct ArchivedDateUtc {
    year: Archived<i32>,
    ordinal: Archived<u32>,
}

impl ArchiveWith<Date<Utc>> for DateWrapper {
    type Archived = ArchivedDateUtc;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &Date<Utc>,
        pos: usize,
        _: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let (fp, fo) = {
            let fo = (&mut (*out).year) as *mut i32;
            (fo.cast::<u8>().offset_from(out.cast::<u8>()) as usize, fo)
        };
        #[allow(clippy::unit_arg)]
        field.year().resolve(pos + fp, (), fo);

        let (fp, fo) = {
            let fo = (&mut (*out).ordinal) as *mut u32;
            (fo.cast::<u8>().offset_from(out.cast::<u8>()) as usize, fo)
        };
        #[allow(clippy::unit_arg)]
        field.ordinal().resolve(pos + fp, (), fo);
    }
}

impl<S: Fallible> SerializeWith<Date<Utc>, S> for DateWrapper {
    #[inline]
    fn serialize_with(_: &Date<Utc>, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: Fallible> DeserializeWith<ArchivedDateUtc, Date<Utc>, D> for DateWrapper {
    #[inline]
    fn deserialize_with(field: &ArchivedDateUtc, _: &mut D) -> Result<Date<Utc>, D::Error> {
        Ok(Utc.yo(field.year, field.ordinal))
    }
}
