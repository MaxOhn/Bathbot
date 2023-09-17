use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_v2::prelude::Score;

use super::{Skills, TitleDescriptions, TitlePrefix, TitleSuffix};

pub(crate) struct CardTitle {
    pub(crate) prefix: TitlePrefix,
    pub(crate) description: TitleDescriptions,
    pub(crate) suffix: TitleSuffix,
}

impl CardTitle {
    pub(crate) fn new(skills: &Skills, scores: &[Score]) -> Self {
        let (max, suffix) = match skills {
            Skills::Osu { acc, aim, speed } => {
                let max = acc.max(*aim).max(*speed);

                (max, TitleSuffix::osu(*acc, *aim, *speed, max))
            }
            Skills::Taiko { acc, strain } => {
                let max = acc.max(*strain);

                (max, TitleSuffix::taiko(*acc, *strain, max))
            }
            Skills::Catch { acc, movement } => {
                let max = acc.max(*movement);

                (max, TitleSuffix::catch(*acc, *movement, max))
            }
            Skills::Mania { acc, strain } => {
                let max = acc.max(*strain);

                (max, TitleSuffix::mania(*acc, *strain, max))
            }
        };

        let prefix = TitlePrefix::new(max);
        let description = TitleDescriptions::new(skills.mode(), scores);

        Self {
            prefix,
            description,
            suffix,
        }
    }
}

impl Display for CardTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.prefix, f)?;

        if !self.description.is_empty() {
            write!(f, " {}", self.description)?;
        }

        write!(f, " {}", self.suffix)
    }
}
