use std::{fmt, marker::PhantomData};

use rosu_v2::prelude::GameMode;
use serde::{
    de::{Deserializer, Error, Unexpected, Visitor},
    ser::Serializer,
    Deserialize,
};
use time::{Date, OffsetDateTime, PrimitiveDateTime};

pub(super) mod option_f32_string {
    use super::{f32_string::F32String, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
        d.deserialize_option(MaybeF32String)
    }

    pub(super) struct MaybeF32String;

    impl<'de> Visitor<'de> for MaybeF32String {
        type Value = Option<f32>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional string containing an f32")
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(F32String).map(Some)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
}

pub(super) mod f32_string {
    use super::{option_f32_string::MaybeF32String, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
        Ok(d.deserialize_option(MaybeF32String)?.unwrap_or(0.0))
    }

    pub(super) struct F32String;

    impl Visitor<'_> for F32String {
        type Value = f32;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string containing an f32")
        }

        fn visit_f32<E: Error>(self, v: f32) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
            Ok(v as f32)
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse()
                .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
        }
    }
}

pub(super) mod option_u32_string {
    use super::{u32_string::U32String, *};

    pub(super) struct MaybeU32String;

    impl<'de> Visitor<'de> for MaybeU32String {
        type Value = Option<u32>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional string containing a u32")
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(U32String).map(Some)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
}

pub(super) mod u32_string {
    use super::{option_u32_string::MaybeU32String, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        Ok(d.deserialize_option(MaybeU32String)?.unwrap_or(0))
    }

    pub(super) struct U32String;

    impl Visitor<'_> for U32String {
        type Value = u32;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string containing a u32")
        }

        fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v as u32)
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse()
                .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
        }
    }
}

pub(super) mod adjust_acc {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
        Ok(<f32 as Deserialize>::deserialize(d)? * 100.0)
    }
}

pub(super) mod naive_datetime {
    use bathbot_util::datetime::NAIVE_DATETIME_FORMAT;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(NaiveDateTimeVisitor)
    }

    pub(super) struct NaiveDateTimeVisitor;

    impl Visitor<'_> for NaiveDateTimeVisitor {
        type Value = OffsetDateTime;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a naive datetime string")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            PrimitiveDateTime::parse(v, NAIVE_DATETIME_FORMAT)
                .map(PrimitiveDateTime::assume_utc)
                .map_err(Error::custom)
        }
    }
}

pub(super) mod option_naive_datetime {
    use super::{naive_datetime::NaiveDateTimeVisitor, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        d.deserialize_option(OptionDateTimeVisitor)
    }

    struct OptionDateTimeVisitor;

    impl<'de> Visitor<'de> for OptionDateTimeVisitor {
        type Value = Option<OffsetDateTime>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional naive datetime string")
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(NaiveDateTimeVisitor).map(Some)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
}

pub(super) struct Datetime(pub OffsetDateTime);

impl<'de> Deserialize<'de> for Datetime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        datetime_rfc3339::deserialize(d).map(Self)
    }
}

pub(super) mod datetime_rfc3339 {
    use time::format_description::well_known::Rfc3339;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(crate) struct DateTimeVisitor;

    impl Visitor<'_> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an RFC3339 datetime string ending on `Z`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            OffsetDateTime::parse(v, &Rfc3339).map_err(Error::custom)
        }
    }
}
pub(super) mod option_datetime_rfc3339 {
    use super::{datetime_rfc3339::DateTimeVisitor, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        d.deserialize_option(OptionDateTimeVisitor)
    }

    struct OptionDateTimeVisitor;

    impl<'de> Visitor<'de> for OptionDateTimeVisitor {
        type Value = Option<OffsetDateTime>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional RFC3339 datetime string ending on `Z`")
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(DateTimeVisitor).map(Some)
        }

        #[inline]
        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            self.visit_unit()
        }

        #[inline]
        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
}

pub(super) mod datetime_rfc2822 {
    use time::format_description::well_known::Rfc2822;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    struct DateTimeVisitor;

    impl Visitor<'_> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an RFC2822 datetime string")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            OffsetDateTime::parse(v, &Rfc2822).map_err(Error::custom)
        }
    }
}

pub(super) mod date {
    use bathbot_util::datetime::DATE_FORMAT;

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        d.deserialize_str(DateVisitor)
    }

    pub(super) struct DateVisitor;

    impl Visitor<'_> for DateVisitor {
        type Value = Date;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a date string")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Date::parse(v, DATE_FORMAT).map_err(Error::custom)
        }
    }
}

pub(super) mod bool_as_u8 {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        match u8::deserialize(d)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(Error::invalid_value(
                Unexpected::Unsigned(other as u64),
                &"0 or 1",
            )),
        }
    }

    pub fn serialize<S: Serializer>(value: &bool, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*value as u8)
    }
}

pub struct ModeAsSeed<T> {
    pub(crate) mode: GameMode,
    phantom: PhantomData<T>,
}

impl<T> Clone for ModeAsSeed<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ModeAsSeed<T> {}

impl<T> ModeAsSeed<T> {
    pub fn new(mode: GameMode) -> Self {
        Self {
            mode,
            phantom: PhantomData,
        }
    }

    pub fn cast<U>(self) -> ModeAsSeed<U> {
        ModeAsSeed::new(self.mode)
    }
}
