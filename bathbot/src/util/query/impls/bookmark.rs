use std::{borrow::Cow, fmt::Write};

use time::Date;

use crate::util::query::{
    operator::Operator,
    optional::{OptionalRange, OptionalText},
    separate_content, IFilterCriteria,
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

        if !bpm.is_empty() {
            separate_content(content);
            let _ = write!(content, "`BPM: {bpm:?}`");
        }

        if !artist.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Artist: {artist:?}`");
        }

        if !title.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Title: {title:?}`");
        }

        if !version.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Version: {version:?}`");
        }

        if !insert_date.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Date: {insert_date:?}`");
        }

        if !ranked_date.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Ranked: {ranked_date:?}`");
        }

        if !language.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Language: {language:?}`");
        }

        if !genre.is_empty() {
            separate_content(content);
            let _ = write!(content, "`Genre: {genre:?}`");
        }
    }
}
