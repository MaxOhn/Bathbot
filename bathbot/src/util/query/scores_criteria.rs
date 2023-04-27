use std::borrow::Cow;

use time::Date;

use super::{
    operator::Operator,
    optional::{OptionalRange, OptionalText},
    IFilterCriteria,
};

#[derive(Default)]
pub struct ScoresCriteria<'q> {
    pub ar: OptionalRange<f32>,
    pub cs: OptionalRange<f32>,
    pub hp: OptionalRange<f32>,
    pub od: OptionalRange<f32>,
    pub length: OptionalRange<f32>,
    pub stars: OptionalRange<f32>,
    pub pp: OptionalRange<f32>,
    pub bpm: OptionalRange<f32>,

    pub combo: OptionalRange<u32>,
    pub miss: OptionalRange<u32>,
    pub score: OptionalRange<u32>,

    pub date: OptionalRange<Date>,
    pub ranked_date: OptionalRange<Date>,

    pub artist: OptionalText<'q>,
    pub title: OptionalText<'q>,
    pub version: OptionalText<'q>,
}

impl<'q> IFilterCriteria<'q> for ScoresCriteria<'q> {
    fn try_parse_keyword_criteria(
        &mut self,
        key: Cow<'q, str>,
        value: Cow<'q, str>,
        op: Operator,
    ) -> bool {
        match key.as_ref() {
            "star" | "stars" => self.stars.try_update(op, &value, 0.005),
            "pp" => self.pp.try_update(op, &value, 0.005),
            "ar" => self.ar.try_update(op, &value, 0.005),
            "dr" | "hp" => self.hp.try_update(op, &value, 0.005),
            "cs" => self.cs.try_update(op, &value, 0.005),
            "od" => self.od.try_update(op, &value, 0.005),
            "bpm" => self.bpm.try_update(op, &value, 0.05),
            "length" | "len" => super::try_update_len(&mut self.length, op, &value),
            "combo" | "maxcombo" => self.combo.try_update(op, &value, 0),
            "score" => self.score.try_update(op, &value, 0),
            "miss" | "nmiss" | "countmiss" | "misses" | "nmisses" => {
                self.miss.try_update(op, &value, 0)
            }
            "date" | "scoredate" | "ended_at" => self.date.try_update_date(op, &value),
            "ranked" | "rankeddate" | "ranked_date" => self.ranked_date.try_update_date(op, &value),
            "artist" => self.artist.try_update(op, value),
            "title" => self.title.try_update(op, value),
            "difficulty" | "version" => self.version.try_update(op, value),
            _ => false,
        }
    }
}
