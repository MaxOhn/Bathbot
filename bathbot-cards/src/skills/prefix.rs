use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub(crate) enum TitlePrefix {
    Newbie,
    Novice,
    Rookie,
    Apprentice,
    Advanced,
    Outstanding,
    Seasoned,
    Professional,
    Expert,
    Master,
    Legendary,
    God,
}

impl TitlePrefix {
    pub(super) fn new(value: f64) -> Self {
        match value {
            _ if value < 10.0 => Self::Newbie,
            _ if value < 20.0 => Self::Novice,
            _ if value < 30.0 => Self::Rookie,
            _ if value < 40.0 => Self::Apprentice,
            _ if value < 50.0 => Self::Advanced,
            _ if value < 60.0 => Self::Outstanding,
            _ if value < 70.0 => Self::Seasoned,
            _ if value < 80.0 => Self::Professional,
            _ if value < 85.0 => Self::Expert,
            _ if value < 90.0 => Self::Master,
            _ if value < 95.0 => Self::Legendary,
            _ => Self::God,
        }
    }

    pub(crate) fn filename(&self) -> &'static str {
        match self {
            Self::Newbie => "newbie",
            Self::Novice => "novice",
            Self::Rookie => "rookie",
            Self::Apprentice => "apprentice",
            Self::Advanced => "advanced",
            Self::Outstanding => "outstanding",
            Self::Seasoned => "seasoned",
            Self::Professional => "professional",
            Self::Expert => "expert",
            Self::Master => "master",
            Self::Legendary => "legendary",
            Self::God => "god",
        }
    }
}

impl Display for TitlePrefix {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Debug>::fmt(self, f)
    }
}
