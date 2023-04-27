use std::borrow::Cow;

use super::{
    operator::Operator,
    optional::{OptionalRange, OptionalText},
    IFilterCriteria,
};

#[derive(Default)]
pub struct RegularCriteria<'q> {
    pub stars: OptionalRange<f32>,
    pub ar: OptionalRange<f32>,
    pub cs: OptionalRange<f32>,
    pub hp: OptionalRange<f32>,
    pub od: OptionalRange<f32>,
    pub length: OptionalRange<f32>,
    pub bpm: OptionalRange<f32>,
    pub keys: OptionalRange<f32>,

    pub artist: OptionalText<'q>,
    pub creator: OptionalText<'q>,
    pub title: OptionalText<'q>,
}

impl<'q> IFilterCriteria<'q> for RegularCriteria<'q> {
    fn try_parse_keyword_criteria(
        &mut self,
        key: Cow<'q, str>,
        value: Cow<'q, str>,
        op: Operator,
    ) -> bool {
        match key.as_ref() {
            "star" | "stars" => self.stars.try_update(op, &value, 0.005),
            "ar" => self.ar.try_update(op, &value, 0.005),
            "dr" | "hp" => self.hp.try_update(op, &value, 0.005),
            "cs" => self.cs.try_update(op, &value, 0.005),
            "od" => self.od.try_update(op, &value, 0.005),
            "bpm" => self.bpm.try_update(op, &value, 0.05),
            "length" | "len" => super::try_update_len(&mut self.length, op, &value),
            "creator" | "mapper" => self.creator.try_update(op, value),
            "artist" => self.artist.try_update(op, value),
            "title" => self.title.try_update(op, value),
            "key" | "keys" => self.keys.try_update(op, &value, 0.5),
            _ => false,
        }
    }
}
