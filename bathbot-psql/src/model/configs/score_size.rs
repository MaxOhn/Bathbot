use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};
use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum ScoreSize {
    #[option(name = "Always minimized", value = "min")]
    AlwaysMinimized = 0,
    #[option(name = "Initial maximized", value = "initial_max")]
    InitialMaximized = 1,
    #[option(name = "Always maximized", value = "max")]
    AlwaysMaximized = 2,
}

impl From<ScoreSize> for i16 {
    #[inline]
    fn from(score_size: ScoreSize) -> Self {
        score_size as Self
    }
}

impl TryFrom<i16> for ScoreSize {
    type Error = ();

    #[inline]
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::AlwaysMinimized),
            1 => Ok(Self::InitialMaximized),
            2 => Ok(Self::AlwaysMaximized),
            _ => Err(()),
        }
    }
}

impl<'q> Encode<'q, Postgres> for ScoreSize {
    #[inline]
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        <i16 as Encode<'q, Postgres>>::encode(*self as i16, buf)
    }
}

impl<'r> Decode<'r, Postgres> for ScoreSize {
    #[inline]
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <i16 as Decode<'r, Postgres>>::decode(value)?;

        Self::try_from(value)
            .map_err(|_| format!("invalid value `{value}` for struct EmbedsSize").into())
    }
}

impl Type<Postgres> for ScoreSize {
    #[inline]
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("INT2")
    }
}

impl Default for ScoreSize {
    #[inline]
    fn default() -> Self {
        Self::InitialMaximized
    }
}
