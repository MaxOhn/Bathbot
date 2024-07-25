use twilight_interactions::command::{CommandOption, CreateOption};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, CommandOption, CreateOption)]
#[repr(u8)]
pub enum ScoreData {
    #[default]
    #[option(name = "Lazer", value = "lazer")]
    Lazer = 1,
    #[option(name = "Stable", value = "stable")]
    Stable = 0,
    #[option(name = "Lazer (Classic scoring)", value = "lazer_classic")]
    LazerWithClassicScoring = 2,
}

impl ScoreData {
    pub fn is_legacy(self) -> bool {
        self == Self::Stable
    }
}

impl From<ScoreData> for i16 {
    fn from(score_data: ScoreData) -> Self {
        score_data as Self
    }
}

impl TryFrom<i16> for ScoreData {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Stable),
            1 => Ok(Self::Lazer),
            2 => Ok(Self::LazerWithClassicScoring),
            _ => Err(()),
        }
    }
}
