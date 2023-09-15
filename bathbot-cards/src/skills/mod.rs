mod description;
mod prefix;
mod suffix;
mod title;

use rosu_v2::model::{score::Score, GameMode};

pub(crate) use self::{
    description::TitleDescriptions, prefix::TitlePrefix, suffix::TitleSuffix, title::CardTitle,
};
use crate::{card::Maps, error::CardError};

pub(crate) enum Skills {
    Osu { acc: f32, aim: f32, speed: f32 },
    Taiko { acc: f32, strain: f32 },
    Catch { acc: f32, movement: f32 },
    Mania { acc: f32, strain: f32 },
}

impl Skills {
    pub(crate) fn calculate<S>(
        mode: GameMode,
        scores: &[Score],
        maps: &Maps<S>,
    ) -> Result<Self, CardError> {
        todo!()
    }

    pub(crate) fn title(&self) -> CardTitle {
        CardTitle::new(self)
    }

    pub(crate) fn mode(&self) -> GameMode {
        match self {
            Skills::Osu { .. } => GameMode::Osu,
            Skills::Taiko { .. } => GameMode::Taiko,
            Skills::Catch { .. } => GameMode::Catch,
            Skills::Mania { .. } => GameMode::Mania,
        }
    }
}
