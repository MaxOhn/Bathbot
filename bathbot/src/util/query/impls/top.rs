use std::borrow::Cow;

use time::Date;

use super::{display_range, display_text};
use crate::util::query::{
    IFilterCriteria,
    operator::Operator,
    optional::{OptionalRange, OptionalText},
};

#[derive(Default)]
pub struct TopCriteria<'q> {
    pub pp: OptionalRange<f32>,
    pub stars: OptionalRange<f32>,
    pub ar: OptionalRange<f32>,
    pub cs: OptionalRange<f32>,
    pub hp: OptionalRange<f32>,
    pub od: OptionalRange<f32>,
    pub length: OptionalRange<f32>,
    pub bpm: OptionalRange<f32>,
    pub acc: OptionalRange<f32>,
    pub score: OptionalRange<u32>,
    pub combo: OptionalRange<u32>,
    pub miss: OptionalRange<u32>,
    pub keys: OptionalRange<f32>,

    pub date: OptionalRange<Date>,
    pub ranked_date: OptionalRange<Date>,

    pub artist: OptionalText<'q>,
    pub creator: OptionalText<'q>,
    pub title: OptionalText<'q>,
    pub version: OptionalText<'q>,
}

impl<'q> IFilterCriteria<'q> for TopCriteria<'q> {
    fn try_parse_key_value(
        &mut self,
        key: Cow<'q, str>,
        value: Cow<'q, str>,
        op: Operator,
    ) -> bool {
        match key.as_ref() {
            "pp" => self.pp.try_update(op, &value, 0.005),
            "star" | "stars" => self.stars.try_update(op, &value, 0.005),
            "ar" => self.ar.try_update(op, &value, 0.005),
            "dr" | "hp" => self.hp.try_update(op, &value, 0.005),
            "cs" => self.cs.try_update(op, &value, 0.005),
            "od" => self.od.try_update(op, &value, 0.005),
            "length" | "len" => super::try_update_len(&mut self.length, op, &value),
            "bpm" => self.bpm.try_update(op, &value, 0.05),
            "acc" | "accuracy" => self.acc.try_update(op, &value, 0.005),
            "score" => self.score.try_update(op, &value, 0),
            "combo" | "maxcombo" => self.combo.try_update(op, &value, 0),
            "miss" | "nmiss" | "countmiss" | "misses" | "nmisses" => {
                self.miss.try_update(op, &value, 0)
            }
            "key" | "keys" => self.keys.try_update(op, &value, 0.5),

            "date" | "scoredate" | "ended_at" => self.date.try_update_date(op, &value),
            "ranked" | "rankeddate" | "ranked_date" => self.ranked_date.try_update_date(op, &value),

            "artist" => self.artist.try_update(op, value),
            "creator" | "mapper" => self.creator.try_update(op, value),
            "version" | "diff" | "difficulty" => self.version.try_update(op, value),
            "title" => self.title.try_update(op, value),
            _ => false,
        }
    }

    fn any_field(&self) -> bool {
        let Self {
            pp,
            stars,
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            acc,
            score,
            combo,
            miss,
            keys,
            date,
            ranked_date,
            artist,
            creator,
            version,
            title,
        } = self;

        !(pp.is_empty()
            && stars.is_empty()
            && ar.is_empty()
            && cs.is_empty()
            && hp.is_empty()
            && od.is_empty()
            && length.is_empty()
            && bpm.is_empty()
            && acc.is_empty()
            && score.is_empty()
            && combo.is_empty()
            && miss.is_empty()
            && keys.is_empty()
            && date.is_empty()
            && ranked_date.is_empty()
            && artist.is_empty()
            && creator.is_empty()
            && version.is_empty()
            && title.is_empty())
    }

    fn display(&self, content: &mut String) {
        let Self {
            pp,
            stars,
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            acc,
            score,
            combo,
            miss,
            keys,
            date,
            ranked_date,
            artist,
            creator,
            version,
            title,
        } = self;

        display_range(content, "AR", ar);
        display_range(content, "CS", cs);
        display_range(content, "HP", hp);
        display_range(content, "OD", od);
        display_range(content, "Length", length);
        display_range(content, "Stars", stars);
        display_range(content, "PP", pp);
        display_range(content, "BPM", bpm);
        display_range(content, "Accuracy", acc);
        display_range(content, "Combo", combo);
        display_range(content, "Misses", miss);
        display_range(content, "Score", score);
        display_range(content, "Keys", keys);

        display_text(content, "Artist", artist);
        display_text(content, "Title", title);
        display_text(content, "Version", version);
        display_text(content, "Creator", creator);

        display_range(content, "Date", date);
        display_range(content, "Ranked", ranked_date);
    }
}
