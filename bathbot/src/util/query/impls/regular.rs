use std::borrow::Cow;

use super::{display_range, display_text};
use crate::util::query::{
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
    fn try_parse_key_value(
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

    fn any_field(&self) -> bool {
        let Self {
            stars,
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            keys,
            artist,
            creator,
            title,
        } = self;

        !(stars.is_empty()
            && ar.is_empty()
            && cs.is_empty()
            && hp.is_empty()
            && od.is_empty()
            && length.is_empty()
            && bpm.is_empty()
            && keys.is_empty()
            && artist.is_empty()
            && creator.is_empty()
            && title.is_empty())
    }

    fn display(&self, content: &mut String) {
        let Self {
            stars,
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            keys,
            artist,
            creator,
            title,
        } = self;

        display_range(content, "AR", ar);
        display_range(content, "CS", cs);
        display_range(content, "HP", hp);
        display_range(content, "OD", od);
        display_range(content, "Length", length);
        display_range(content, "Stars", stars);
        display_range(content, "BPM", bpm);
        display_range(content, "Keys", keys);
        display_range(content, "AR", ar);
        display_range(content, "AR", ar);

        display_text(content, "Artist", artist);
        display_text(content, "Title", title);
        display_text(content, "Creator", creator);
    }
}
