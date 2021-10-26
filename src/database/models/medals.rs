use rosu_v2::model::GameMode;
use sqlx::FromRow;
use std::{cmp::Ordering, fmt, str::FromStr};

#[derive(Clone, Debug, FromRow)]
pub struct DBOsuMedal {
    pub medal_id: i32,
    pub name: String,
    pub description: String,
    pub grouping: String,
    pub icon_url: String,
    pub instructions: Option<String>,
    pub mode: Option<i16>,
}

#[derive(Clone, Debug)]
pub struct OsuMedal {
    pub medal_id: u32,
    pub name: String,
    pub description: String,
    pub grouping: MedalGroup,
    pub icon_url: String,
    pub instructions: Option<String>,
    pub mode: Option<GameMode>,
}

impl PartialEq for OsuMedal {
    fn eq(&self, other: &Self) -> bool {
        self.medal_id == other.medal_id
    }
}

impl Eq for OsuMedal {}

impl PartialOrd for OsuMedal {
    fn partial_cmp(&self, other: &OsuMedal) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OsuMedal {
    fn cmp(&self, other: &OsuMedal) -> Ordering {
        (self.grouping)
            .cmp(&other.grouping)
            .then_with(|| self.medal_id.cmp(&other.medal_id))
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum MedalGroup {
    Skill = 0,
    Dedication = 1,
    HushHush = 2,
    BeatmapPacks = 3,
    BeatmapChallengePacks = 4,
    SeasonalSpotlights = 5,
    BeatmapSpotlights = 6,
    ModIntroduction = 7,
}

impl fmt::Display for MedalGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skill => f.write_str("Skill"),
            Self::Dedication => f.write_str("Dedication"),
            Self::HushHush => f.write_str("Hush-Hush"),
            Self::BeatmapPacks => f.write_str("Beatmap Packs"),
            Self::BeatmapChallengePacks => f.write_str("Beatmap Challenge Packs"),
            Self::SeasonalSpotlights => f.write_str("Seasonal Spotlights"),
            Self::BeatmapSpotlights => f.write_str("Beatmap Spotlights"),
            Self::ModIntroduction => f.write_str("Mod Introduction"),
        }
    }
}

impl FromStr for MedalGroup {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Skill" => Ok(Self::Skill),
            "Dedication" => Ok(Self::Dedication),
            "Hush-Hush" => Ok(Self::HushHush),
            "Beatmap Packs" => Ok(Self::BeatmapPacks),
            "Beatmap Challenge Packs" => Ok(Self::BeatmapChallengePacks),
            "Seasonal Spotlights" => Ok(Self::SeasonalSpotlights),
            "Beatmap Spotlights" => Ok(Self::BeatmapSpotlights),
            "Mod Introduction" => Ok(Self::ModIntroduction),
            _ => Err(()),
        }
    }
}

impl PartialOrd for MedalGroup {
    fn partial_cmp(&self, other: &MedalGroup) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MedalGroup {
    fn cmp(&self, other: &MedalGroup) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl From<DBOsuMedal> for OsuMedal {
    fn from(medal: DBOsuMedal) -> Self {
        let grouping: MedalGroup = match medal.grouping.parse() {
            Ok(group) => group,
            Err(_) => panic!("Failed to parse `{}` as MedalGroup", medal.grouping),
        };

        Self {
            medal_id: medal.medal_id as u32,
            name: medal.name,
            description: medal.description,
            grouping,
            icon_url: medal.icon_url,
            instructions: medal.instructions,
            mode: medal.mode.map(|m| GameMode::from(m as u8)),
        }
    }
}
