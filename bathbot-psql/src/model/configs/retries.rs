use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum Retries {
    #[option(name = "Hide", value = "hide")]
    Hide = 0,
    #[option(name = "Consider mods", value = "with_mods")]
    ConsiderMods = 1,
    #[option(name = "Ignore mods", value = "ignore_mods")]
    IgnoreMods = 2,
}

impl From<Retries> for i16 {
    #[inline]
    fn from(retries: Retries) -> Self {
        retries as Self
    }
}

impl TryFrom<i16> for Retries {
    type Error = ();

    #[inline]
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Hide),
            1 => Ok(Self::ConsiderMods),
            2 => Ok(Self::IgnoreMods),
            _ => Err(()),
        }
    }
}
