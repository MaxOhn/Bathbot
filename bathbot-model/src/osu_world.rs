use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::{Formatter, Result as FmtResult},
    hash::{Hash, Hasher},
};

use compact_str::CompactString;
use serde::{
    de::{Error as DeError, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};

#[derive(Copy, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Eq, PartialEq)]
#[archive(as = "Self")]
pub struct CountryCode([u8; 2]);

impl CountryCode {
    pub fn as_str(&self) -> &str {
        // SAFETY: self.0 is only created via deserialization
        // which always originates from valid strings
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }
}

impl Hash for CountryCode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Borrow<str> for CountryCode {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<'de> Deserialize<'de> for CountryCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct CountryCodeVisitor;

        impl<'de> Visitor<'de> for CountryCodeVisitor {
            type Value = CountryCode;

            fn expecting(&self, f: &mut Formatter) -> FmtResult {
                f.write_str("a country code string")
            }

            fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
                v.as_bytes().try_into().map(CountryCode).map_err(|_| {
                    let expected = "a country code consisting of two ASCII letters";

                    DeError::invalid_value(Unexpected::Str(v), &expected)
                })
            }
        }

        d.deserialize_str(CountryCodeVisitor)
    }
}

pub type RegionCode = CompactString;
pub type RegionName = CompactString;
pub type Regions = HashMap<RegionCode, RegionName>;
pub type CountryRegions = HashMap<CountryCode, Regions>;

pub struct OsuWorldUserIds(pub Vec<i32>);

impl<'de> Deserialize<'de> for OsuWorldUserIds {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct OsuWorldUserIdsVisitor;

        impl<'de> Visitor<'de> for OsuWorldUserIdsVisitor {
            type Value = OsuWorldUserIds;

            fn expecting(&self, f: &mut Formatter) -> FmtResult {
                f.write_str("a sequence of osuworld users")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                struct UserId(i32);

                impl<'de> Deserialize<'de> for UserId {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct UserIdVisitor;

                        impl<'de> Visitor<'de> for UserIdVisitor {
                            type Value = UserId;

                            fn expecting(&self, f: &mut Formatter) -> FmtResult {
                                f.write_str("a user id item")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                let mut user_id = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "id" => user_id = Some(map.next_value()?),
                                        _ => {
                                            let _: IgnoredAny = map.next_value()?;
                                        }
                                    }
                                }

                                user_id
                                    .ok_or_else(|| A::Error::missing_field("id"))
                                    .map(UserId)
                            }
                        }

                        d.deserialize_map(UserIdVisitor)
                    }
                }

                let mut user_ids = Vec::new();

                while let Some(UserId(id)) = seq.next_element()? {
                    user_ids.push(id);
                }

                Ok(OsuWorldUserIds(user_ids))
            }
        }

        d.deserialize_seq(OsuWorldUserIdsVisitor)
    }
}
