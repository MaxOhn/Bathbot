use std::borrow::Cow;

use time::Date;

use super::{display_range, display_text};
use crate::query::{
    IFilterCriteria,
    operator::Operator,
    optional::{OptionalRange, OptionalText},
};

#[derive(Default)]
pub struct BookmarkCriteria<'q> {
    pub ar: OptionalRange<f32>,
    pub cs: OptionalRange<f32>,
    pub hp: OptionalRange<f32>,
    pub od: OptionalRange<f32>,
    pub length: OptionalRange<f32>,
    pub bpm: OptionalRange<f32>,

    pub insert_date: OptionalRange<Date>,
    pub ranked_date: OptionalRange<Date>,

    pub artist: OptionalText<'q>,
    pub title: OptionalText<'q>,
    pub version: OptionalText<'q>,

    pub language: OptionalText<'q>,
    pub genre: OptionalText<'q>,
}

impl<'q> IFilterCriteria<'q> for BookmarkCriteria<'q> {
    fn try_parse_key_value(
        &mut self,
        key: Cow<'q, str>,
        value: Cow<'q, str>,
        op: Operator,
    ) -> bool {
        match key.as_ref() {
            "ar" => self.ar.try_update(op, &value, 0.005),
            "dr" | "hp" => self.hp.try_update(op, &value, 0.005),
            "cs" => self.cs.try_update(op, &value, 0.005),
            "od" => self.od.try_update(op, &value, 0.005),
            "bpm" => self.bpm.try_update(op, &value, 0.05),
            "length" | "len" => super::try_update_len(&mut self.length, op, &value),
            "ranked" | "rankeddate" | "ranked_date" => self.ranked_date.try_update_date(op, &value),
            "bookmarked" | "bookmarkdate" | "bookmark_date" | "insertdate" | "insert_date" => {
                self.insert_date.try_update_date(op, &value)
            }
            "artist" => self.artist.try_update(op, value),
            "title" => self.title.try_update(op, value),
            "difficulty" | "version" | "diff" => self.version.try_update(op, value),
            "language" | "lang" => self.language.try_update(op, value),
            "genre" => self.genre.try_update(op, value),
            _ => false,
        }
    }

    fn any_field(&self) -> bool {
        let Self {
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            insert_date,
            ranked_date,
            artist,
            title,
            version,
            language,
            genre,
        } = self;

        !(ar.is_empty()
            && cs.is_empty()
            && hp.is_empty()
            && od.is_empty()
            && length.is_empty()
            && bpm.is_empty()
            && insert_date.is_empty()
            && ranked_date.is_empty()
            && artist.is_empty()
            && title.is_empty()
            && version.is_empty()
            && language.is_empty()
            && genre.is_empty())
    }

    fn display(&self, content: &mut String) {
        let Self {
            ar,
            cs,
            hp,
            od,
            length,
            bpm,
            insert_date,
            ranked_date,
            artist,
            title,
            version,
            language,
            genre,
        } = self;

        display_range(content, "AR", ar);
        display_range(content, "CS", cs);
        display_range(content, "HP", hp);
        display_range(content, "OD", od);
        display_range(content, "Length", length);
        display_range(content, "BPM", bpm);

        display_text(content, "Artist", artist);
        display_text(content, "Title", title);
        display_text(content, "Version", version);

        display_range(content, "Date", insert_date);
        display_range(content, "Ranked", ranked_date);

        display_text(content, "Language", language);
        display_text(content, "Genre", genre);
    }
}
