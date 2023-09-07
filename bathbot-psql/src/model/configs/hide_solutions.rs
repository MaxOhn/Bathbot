use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum HideSolutions {
    #[option(name = "Show all solutions", value = "show")]
    ShowAll = 0,
    #[option(name = "Hide Hush-Hush solutions", value = "hide_hushhush")]
    HideHushHush = 1,
    #[option(name = "Hide all solutions", value = "hide_all")]
    HideAll = 2,
}

impl From<HideSolutions> for i16 {
    #[inline]
    fn from(hide_solutions: HideSolutions) -> Self {
        hide_solutions as Self
    }
}

impl TryFrom<i16> for HideSolutions {
    type Error = ();

    #[inline]
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ShowAll),
            1 => Ok(Self::HideHushHush),
            2 => Ok(Self::HideAll),
            _ => Err(()),
        }
    }
}
