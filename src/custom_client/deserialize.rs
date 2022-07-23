use std::{fmt, str::FromStr};

use rosu_v2::model::GameMods;
use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use time::{Date, OffsetDateTime, PrimitiveDateTime};

use crate::util::datetime::{DATETIME_FORMAT, DATE_FORMAT, OFFSET_DATETIME_FORMAT};

pub(super) mod option_f32_string {
    use super::{f32_string::F32String, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
        d.deserialize_option(MaybeF32String)
    }

    pub(super) struct MaybeF32String;

    impl<'de> Visitor<'de> for MaybeF32String {
        type Value = Option<f32>;

        #[inline]
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

    impl<'de> Visitor<'de> for F32String {
        type Value = f32;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string containing an f32")
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

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional string containing an u32")
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

    impl<'de> Visitor<'de> for U32String {
        type Value = u32;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string containing an u32")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse()
                .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
        }
    }
}

pub(super) mod option_mods_string {
    use super::{mods_string::ModsString, *};

    pub(super) struct MaybeModsString;

    impl<'de> Visitor<'de> for MaybeModsString {
        type Value = Option<GameMods>;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional string containing gamemods")
        }

        #[inline]
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(ModsString).map(Some)
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

pub(super) mod mods_string {
    use super::{option_mods_string::MaybeModsString, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<GameMods, D::Error> {
        Ok(d.deserialize_option(MaybeModsString)?.unwrap_or_default())
    }

    pub(super) struct ModsString;

    impl<'de> Visitor<'de> for ModsString {
        type Value = GameMods;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string containing gamemods")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            let mut mods = GameMods::NoMod;

            if v == "None" {
                return Ok(mods);
            }

            for result in v.split(',').map(GameMods::from_str) {
                match result {
                    Ok(m) => mods |= m,
                    Err(err) => {
                        return Err(Error::custom(format_args!(r#"invalid value "{v}": {err}"#)));
                    }
                }
            }

            Ok(mods)
        }
    }
}

pub(super) mod negative_u32 {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        Ok(<i32 as Deserialize>::deserialize(d)?.max(0) as u32)
    }
}

pub(super) mod adjust_acc {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
        Ok(<f32 as Deserialize>::deserialize(d)? * 100.0)
    }
}

pub(super) mod datetime_maybe_offset {
    use time::error::{Parse, ParseFromDescription};

    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an `OffsetDateTime`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            match OffsetDateTime::parse(v, OFFSET_DATETIME_FORMAT) {
                Ok(datetime) => Ok(datetime),
                Err(
                    err @ Parse::ParseFromDescription(ParseFromDescription::InvalidComponent(_)),
                ) => Err(Error::custom(err)),
                Err(_) => PrimitiveDateTime::parse(v, DATETIME_FORMAT)
                    .map(PrimitiveDateTime::assume_utc)
                    .map_err(Error::custom),
            }
        }
    }
}

pub(super) mod datetime {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an `OffsetDateTime`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            PrimitiveDateTime::parse(v, DATETIME_FORMAT)
                .map(PrimitiveDateTime::assume_utc)
                .map_err(Error::custom)
        }
    }
}

pub(super) mod option_datetime {
    use super::{datetime::DateTimeVisitor, *};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        d.deserialize_option(OptionDateTimeVisitor)
    }

    struct OptionDateTimeVisitor;

    impl<'de> Visitor<'de> for OptionDateTimeVisitor {
        type Value = Option<OffsetDateTime>;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an optional `OffsetDateTime`")
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

pub(super) mod offset_datetime {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        d.deserialize_str(DateTimeVisitor)
    }

    pub(super) struct DateTimeVisitor;

    impl<'de> Visitor<'de> for DateTimeVisitor {
        type Value = OffsetDateTime;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an `OffsetDateTime`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            OffsetDateTime::parse(v, OFFSET_DATETIME_FORMAT).map_err(Error::custom)
        }
    }
}

pub(super) mod date {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        d.deserialize_str(DateVisitor)
    }

    pub(super) struct DateVisitor;

    impl<'de> Visitor<'de> for DateVisitor {
        type Value = Date;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a `Date`")
        }

        #[inline]
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Date::parse(v, DATE_FORMAT).map_err(Error::custom)
        }
    }
}
