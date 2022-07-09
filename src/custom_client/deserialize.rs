use std::{fmt, str::FromStr};

use rosu_v2::model::GameMods;
use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer,
};

// pub fn str_to_maybe_datetime<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     d.deserialize_option(MaybeDateTimeString)
// }

// struct MaybeDateTimeString;

// impl<'de> Visitor<'de> for MaybeDateTimeString {
//     type Value = Option<DateTime<Utc>>;

//     fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.write_str("a string containing a datetime")
//     }

//     fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
//         match Utc.datetime_from_str(v, DATE_FORMAT) {
//             Ok(date) => Ok(Some(date)),
//             Err(_) => Ok(None),
//         }
//     }

//     fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
//         d.deserialize_str(self)
//     }

//     fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
//         Ok(None)
//     }
// }

// pub fn str_to_datetime<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
//     Ok(str_to_maybe_datetime(d)?.unwrap())
// }

// pub fn str_to_date<'de, D: Deserializer<'de>>(d: D) -> Result<Date<Utc>, D::Error> {
//     let date: NaiveDate = Deserialize::deserialize(d)?;

//     Ok(Date::from_utc(date, Utc))
// }

pub fn str_to_maybe_f32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    d.deserialize_option(MaybeF32String)
}

struct MaybeF32String;

impl<'de> Visitor<'de> for MaybeF32String {
    type Value = Option<f32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing an f32")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn str_to_f32<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(str_to_maybe_f32(d)?.unwrap_or(0.0))
}

pub fn str_to_maybe_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    d.deserialize_option(MaybeU32String)
}

struct MaybeU32String;

impl<'de> Visitor<'de> for MaybeU32String {
    type Value = Option<u32>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing an u32")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn str_to_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(str_to_maybe_u32(d)?.unwrap_or(0))
}

pub fn adjust_mods_maybe<'de, D: Deserializer<'de>>(d: D) -> Result<Option<GameMods>, D::Error> {
    d.deserialize_option(MaybeModsString)
}

struct MaybeModsString;

impl<'de> Visitor<'de> for MaybeModsString {
    type Value = Option<GameMods>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string containing gamemods")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let mut mods = GameMods::NoMod;

        if v == "None" {
            return Ok(Some(mods));
        }

        for result in v.split(',').map(GameMods::from_str) {
            match result {
                Ok(m) => mods |= m,
                Err(err) => {
                    return Err(Error::custom(format_args!(r#"invalid value "{v}": {err}"#)));
                }
            }
        }

        Ok(Some(mods))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_str(self)
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

pub fn adjust_mods<'de, D: Deserializer<'de>>(d: D) -> Result<GameMods, D::Error> {
    Ok(adjust_mods_maybe(d)?.unwrap_or_default())
}

pub fn expect_negative_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let i: i64 = Deserialize::deserialize(d)?;

    Ok(i.max(0) as u32)
}

pub fn inflate_acc<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let acc: f32 = Deserialize::deserialize(d)?;

    Ok(100.0 * acc)
}

pub(super) mod datetime {
    use std::fmt;

    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::{OffsetDateTime, PrimitiveDateTime};

    use crate::util::datetime::DATETIME_FORMAT;

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
    use std::fmt;

    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::OffsetDateTime;

    use super::datetime::DateTimeVisitor;

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
    use std::fmt;

    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::OffsetDateTime;

    use crate::util::datetime::OFFSET_DATETIME_FORMAT;

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
    use std::fmt;

    use serde::{
        de::{Error, Visitor},
        Deserializer,
    };
    use time::Date;

    use crate::util::datetime::DATE_FORMAT;

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
