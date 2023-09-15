use super::{Skills, TitleDescriptions, TitlePrefix, TitleSuffix};

pub(crate) struct CardTitle {
    pub(crate) prefix: TitlePrefix,
    pub(crate) description: TitleDescriptions,
    pub(crate) suffix: TitleSuffix,
}

impl CardTitle {
    pub(crate) fn new(skills: &Skills) -> Self {
        todo!()
    }
}
