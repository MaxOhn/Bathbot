use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::{Formatter, Result as FmtResult},
    hash::{Hash, Hasher},
};

use compact_str::CompactString;
use rkyv::{
    bytecheck::CheckBytes,
    collections::util::{Entry, EntryAdapter},
    rancor::{Fallible, Source},
    ser::{Allocator, Writer},
    string::{ArchivedString, StringResolver},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, SerializeWith, With},
    Archive, Place, Portable, Serialize,
};
use serde::{
    de::{Error as DeError, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};

#[derive(
    Copy,
    Clone,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Eq,
    PartialEq,
    Portable,
    CheckBytes,
)]
#[rkyv(as = Self)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(transparent)]
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

#[derive(Archive, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct CountryRegions(HashMap<CountryCode, Regions>);

impl ArchivedCountryRegions {
    pub fn get(&self, country_code: &str) -> Option<&ArchivedRegions> {
        self.0.get(country_code)
    }
}

#[derive(Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
struct Regions(HashMap<RegionCode, RegionName>);

pub type ArchivedRegions = ArchivedVec<Entry<ArchivedString, ArchivedString>>;

impl Archive for Regions {
    type Archived = ArchivedRegions;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(self.0.len(), resolver, out);
    }
}

impl<S> Serialize<S> for Regions
where
    S: Fallible<Error: Source> + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        struct AsString;

        impl ArchiveWith<CompactString> for AsString {
            type Archived = ArchivedString;
            type Resolver = StringResolver;

            fn resolve_with(
                field: &CompactString,
                resolver: Self::Resolver,
                out: Place<Self::Archived>,
            ) {
                ArchivedString::resolve_from_str(field, resolver, out);
            }
        }

        impl<S> SerializeWith<CompactString, S> for AsString
        where
            S: Fallible<Error: Source> + Writer + ?Sized,
        {
            fn serialize_with(
                field: &CompactString,
                serializer: &mut S,
            ) -> Result<Self::Resolver, <S as Fallible>::Error> {
                ArchivedString::serialize_from_str(field, serializer)
            }
        }

        type CompactAsString = With<CompactString, AsString>;

        ArchivedVec::serialize_from_iter(
            self.0.iter().map(|(key, value)| {
                EntryAdapter::<_, _, CompactAsString, CompactAsString>::new(
                    CompactAsString::cast(key),
                    CompactAsString::cast(value),
                )
            }),
            serializer,
        )
    }
}

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
