macro_rules! define_enum {
    (
        #[$enum_meta:meta]
        pub enum $enum_name:ident {
            $(
                $( #[$variant_meta:meta] )?
                $variant:ident = $discriminant:literal,
            )*
        }
    ) => {
        #[$enum_meta]
        pub enum $enum_name {
            $(
                $( #[$variant_meta] )?
                $variant = $discriminant,
            )*
        }

        impl<'de> Deserialize<'de> for $enum_name {
            fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                match u8::deserialize(d)? {
                    $( $discriminant => Ok(Self::$variant), )*
                    other => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(u64::from(other)),
                        &stringify!($enum_name),
                    )),
                }
            }
        }

        impl Serialize for $enum_name {
            fn serialize<S: serde::ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_u8(*self as u8)
            }
        }
    }
}

mod settings;
mod value;

pub use self::{settings::*, value::*};

fn is_true(b: &bool) -> bool {
    *b
}
