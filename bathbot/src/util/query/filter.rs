use std::{borrow::Cow, cmp::Ordering, fmt};

use crate::util::{matcher::QUERY_SYNTAX_REGEX, CowUtils};

#[derive(Debug, Default)]
pub struct FilterCriteria<'q> {
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

    search_text: String,
}

impl<'q> FilterCriteria<'q> {
    pub fn new(query: &'q str) -> Self {
        let mut criteria = Self {
            search_text: query.to_owned(),
            ..Default::default()
        };

        let mut removed = 0;

        for capture in QUERY_SYNTAX_REGEX.get().captures_iter(query) {
            let key_match = match capture.name("key") {
                Some(key) => key,
                None => continue,
            };

            let value_match = match capture.name("value") {
                Some(value) => value,
                None => continue,
            };

            let key = key_match.as_str().cow_to_ascii_lowercase();
            let op = Operator::from(&capture["op"]);
            let value = value_match.as_str().cow_to_ascii_lowercase();

            if criteria.try_parse_keyword_criteria(key, value, op) {
                let range = key_match.start() - removed..value_match.end() - removed;
                criteria.search_text.replace_range(range, "");
                removed += value_match.end() - key_match.start();
            }
        }

        // Index of the last non-whitespace char
        let mut trunc_idx = criteria
            .search_text
            .char_indices()
            .rev()
            .find_map(|(i, c)| (!c.is_whitespace()).then(|| i + c.len_utf8()))
            .unwrap_or(0);

        // Index of the first non-whitespace char
        let start = criteria
            .search_text
            .char_indices()
            .find_map(|(i, c)| (!c.is_whitespace()).then_some(i))
            .filter(|&i| i > 0);

        // If there is whitespace at the front, rotate to the left until
        // the string starts with the first non-whitespace char
        if let Some(shift) = start {
            // SAFETY: The shift is given by .char_indices which is a valid idx
            unsafe { criteria.search_text.as_bytes_mut() }.rotate_left(shift);
            trunc_idx -= shift;
        }

        // Truncate the whitespace
        if trunc_idx < criteria.search_text.len() {
            criteria.search_text.truncate(trunc_idx);
        }

        criteria.search_text.make_ascii_lowercase();

        criteria
    }

    pub fn has_search_terms(&self) -> bool {
        !self.search_text.is_empty()
    }

    pub fn search_terms(&self) -> impl Iterator<Item = &str> {
        self.search_text.split_whitespace()
    }

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
            "length" | "len" => self.try_update_len(op, &value),
            "creator" | "mapper" => self.creator.try_update(op, value),
            "artist" => self.creator.try_update(op, value),
            "title" => self.title.try_update(op, value),
            "key" | "keys" => self.keys.try_update(op, &value, 0.5),
            _ => false,
        }
    }

    fn try_update_len(&mut self, op: Operator, value: &str) -> bool {
        let len: f32 = match value.trim_end_matches(&['m', 's', 'h']).parse() {
            Ok(value) => value,
            Err(_) => return false,
        };

        let scale = if value.ends_with("ms") {
            1.0
        } else if value.ends_with('s') {
            1000.0
        } else if value.ends_with('m') {
            60_000.0
        } else if value.ends_with('h') {
            3_600_000.0
        } else {
            1000.0
        };

        self.length.try_update_(op, len * scale, scale / 2.0)
    }
}

enum Operator {
    Equal,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl From<&str> for Operator {
    fn from(s: &str) -> Self {
        match s {
            "=" | ":" => Self::Equal,
            "<" => Self::Less,
            "<=" | "<:" => Self::LessOrEqual,
            ">" => Self::Greater,
            ">=" | ">:" => Self::GreaterOrEqual,
            _ => unreachable!(),
        }
    }
}

#[derive(Default)]
pub struct OptionalText<'q> {
    search_term: Cow<'q, str>,
}

impl fmt::Debug for OptionalText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.search_term.is_empty() {
            f.write_str("None")
        } else {
            write!(f, "Some({})", self.search_term)
        }
    }
}

impl<'q> OptionalText<'q> {
    pub fn matches(&self, value: &str) -> bool {
        self.search_term.is_empty() || self.search_term == value.cow_to_ascii_lowercase()
    }

    fn try_update(&mut self, op: Operator, value: Cow<'q, str>) -> bool {
        match op {
            Operator::Equal => {
                self.search_term = match value {
                    Cow::Borrowed(b) => b.trim_matches('"').into(),
                    Cow::Owned(o) => {
                        let trimmed = o.trim_matches('"');

                        if trimmed.len() == o.len() {
                            Cow::Owned(o)
                        } else {
                            Cow::Owned(trimmed.to_owned())
                        }
                    }
                };

                true
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct OptionalRange<T> {
    min: Option<T>,
    max: Option<T>,

    is_lower_inclusive: bool,
    is_upper_inclusive: bool,
}

impl<T: fmt::Display> fmt::Debug for OptionalRange<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min.is_none()
            && self.max.is_none()
            && !self.is_lower_inclusive
            && !self.is_upper_inclusive
        {
            return f.write_str("..");
        }

        if self.is_lower_inclusive {
            f.write_str("[")?;
        } else {
            f.write_str("(")?;
        }

        if let Some(ref min) = self.min {
            write!(f, "{min}")?;
        }

        f.write_str(",")?;

        if let Some(ref max) = self.max {
            write!(f, "{max}")?;
        }

        if self.is_upper_inclusive {
            f.write_str("]")?;
        } else {
            f.write_str(")")?;
        }

        Ok(())
    }
}

impl OptionalRange<f32> {
    fn try_update(&mut self, op: Operator, value: &str, tolerance: f32) -> bool {
        let value: f32 = match value.parse() {
            Ok(value) => value,
            Err(_) => return false,
        };

        self.try_update_(op, value, tolerance)
    }

    fn try_update_(&mut self, op: Operator, value: f32, tolerance: f32) -> bool {
        match op {
            Operator::Equal => {
                self.min = Some(value - tolerance);
                self.max = Some(value + tolerance);
            }
            Operator::Less => self.max = Some(value - tolerance),
            Operator::LessOrEqual => self.max = Some(value + tolerance),
            Operator::Greater => self.min = Some(value + tolerance),
            Operator::GreaterOrEqual => self.min = Some(value - tolerance),
        }

        true
    }
}

impl<T: PartialOrd> OptionalRange<T> {
    pub fn contains(&self, value: T) -> bool {
        if let Some(ref min) = self.min {
            match value.partial_cmp(min) {
                Some(Ordering::Less) | None => return false,
                Some(Ordering::Equal) => return self.is_lower_inclusive,
                Some(Ordering::Greater) => {}
            }
        }

        if let Some(ref max) = self.max {
            match value.partial_cmp(max) {
                Some(Ordering::Less) => {}
                Some(Ordering::Equal) => return self.is_lower_inclusive,
                Some(Ordering::Greater) | None => return false,
            }
        }

        true
    }
}
