use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};
use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum MinimizedPp {
    #[option(name = "If FC", value = "if_fc")]
    IfFc = 0,
    #[option(name = "Max PP", value = "max")]
    MaxPp = 1,
}

impl From<MinimizedPp> for i16 {
    #[inline]
    fn from(val: MinimizedPp) -> Self {
        val as Self
    }
}

impl TryFrom<i16> for MinimizedPp {
    type Error = ();

    #[inline]
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::IfFc),
            1 => Ok(Self::MaxPp),
            _ => Err(()),
        }
    }
}

impl<'q> Encode<'q, Postgres> for MinimizedPp {
    #[inline]
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        <i16 as Encode<'q, Postgres>>::encode(*self as i16, buf)
    }
}

impl<'r> Decode<'r, Postgres> for MinimizedPp {
    #[inline]
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <i16 as Decode<'r, Postgres>>::decode(value)?;

        Self::try_from(value)
            .map_err(|_| format!("invalid value `{value}` for struct MinimizedPp").into())
    }
}

impl Type<Postgres> for MinimizedPp {
    #[inline]
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("INT2")
    }
}

impl Default for MinimizedPp {
    #[inline]
    fn default() -> Self {
        Self::MaxPp
    }
}
