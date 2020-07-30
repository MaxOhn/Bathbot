use serde::de::{Error as DeError, Visitor};
use serde::Deserializer;
use std::fmt;

pub fn deserialize_u16<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    deserializer.deserialize_any(U16Visitor)
}

#[derive(Debug)]
pub struct U16Visitor;

impl<'de> Visitor<'de> for U16Visitor {
    type Value = u16;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("identifier")
    }

    fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
        v.parse::<u16>().map_err(|_| {
            let mut s = String::with_capacity(32);
            s.push_str("Unknown u16 value: ");
            s.push_str(v);
            DeError::custom(s)
        })
    }

    fn visit_i64<E: DeError>(self, v: i64) -> Result<Self::Value, E> {
        Ok(v as u16)
    }

    fn visit_u64<E: DeError>(self, v: u64) -> Result<Self::Value, E> {
        Ok(v as u16)
    }
}
