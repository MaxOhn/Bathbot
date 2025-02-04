use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};
use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum ListSize {
    #[option(name = "Condensed", value = "condensed")]
    Condensed = 0,
    #[option(name = "Detailed", value = "detailed")]
    Detailed = 1,
    #[option(name = "Single", value = "single")]
    Single = 2,
}

impl From<ListSize> for i16 {
    #[inline]
    fn from(list_size: ListSize) -> Self {
        list_size as Self
    }
}

impl TryFrom<i16> for ListSize {
    type Error = ();

    #[inline]
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Condensed),
            1 => Ok(Self::Detailed),
            2 => Ok(Self::Single),
            _ => Err(()),
        }
    }
}

impl<'q> Encode<'q, Postgres> for ListSize {
    #[inline]
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <i16 as Encode<'q, Postgres>>::encode(*self as i16, buf)
    }

    fn encode(
        self,
        buf: &mut <Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError>
    where
        Self: Sized,
    {
        self.encode_by_ref(buf)
    }

    fn produces(&self) -> Option<<Postgres as sqlx::Database>::TypeInfo> {
        // `produces` is inherently a hook to allow database drivers to produce
        // value-dependent type information; if the driver doesn't need this, it
        // can leave this as `None`
        None
    }

    fn size_hint(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

impl<'r> Decode<'r, Postgres> for ListSize {
    #[inline]
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <i16 as Decode<'r, Postgres>>::decode(value)?;

        Self::try_from(value)
            .map_err(|_| format!("invalid value `{value}` for struct ListSize").into())
    }
}

impl Type<Postgres> for ListSize {
    #[inline]
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("INT2")
    }
}

impl Default for ListSize {
    #[inline]
    fn default() -> Self {
        Self::Condensed
    }
}
