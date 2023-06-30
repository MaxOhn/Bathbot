use std::{borrow::Cow, fmt::Write};

use crate::util::query::{
    operator::Operator,
    optional::{OptionalRange, OptionalText},
    separate_content, IFilterCriteria,
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

        if !ar.is_empty() {
            separate_content(content);
            let _ = write!(content, "`AR: {ar:?}`");
        }

        if !cs.is_empty() {
            separate_content(content);
            let _ = write!(content, "`CS: {cs:?}`");
        }

        if !hp.is_empty() {
            separate_content(content);
            let _ = write!(content, "`HP: {hp:?}`");
        }

        if !od.is_empty() {
            separate_content(content);
            let _ = write!(content, "`OD: {od:?}`");
        }

        if !length.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Length: {length:?}`");
        }

        if !stars.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Stars: {stars:?}`");
        }

        if !bpm.is_empty() {
            separate_content(content);
            let _ = write!(content, "`BPM: {bpm:?}`");
        }

        if !keys.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Keys: {keys:?}`");
        }

        if !artist.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Artist: {artist:?}`");
        }

        if !title.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Title: {title:?}`");
        }

        if !creator.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Creator: {creator:?}`");
        }
    }
}
