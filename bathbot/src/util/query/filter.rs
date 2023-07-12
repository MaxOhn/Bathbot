use std::{borrow::Cow, ops::Deref};

use bathbot_util::{matcher::QUERY_SYNTAX_REGEX, CowUtils};

use super::{operator::Operator, separate_content};

pub trait IFilterCriteria<'q>: Sized + Default {
    fn try_parse_key_value(&mut self, key: Cow<'q, str>, value: Cow<'q, str>, op: Operator)
        -> bool;

    /// Whether the criteria has any non-empty field
    fn any_field(&self) -> bool;

    fn display(&self, content: &mut String);

    fn create(query: &'q str) -> FilterCriteria<Self> {
        FilterCriteria::new(query)
    }
}

#[derive(Default)]
pub struct FilterCriteria<F> {
    inner: F,
    search_text: String,
}

impl<'q, F: IFilterCriteria<'q>> FilterCriteria<F> {
    fn new(query: &'q str) -> Self {
        let mut search_text = query.to_owned();
        let mut inner = F::default();
        let mut removed = 0;

        for capture in QUERY_SYNTAX_REGEX.get().captures_iter(query) {
            let Some(key_match) = capture.name("key") else {
                continue;
            };

            let Some(value_match) = capture.name("value") else {
                continue;
            };

            let key = key_match.as_str().cow_to_ascii_lowercase();
            let op = Operator::from(&capture["op"]);
            let value = value_match.as_str().cow_to_ascii_lowercase();

            if inner.try_parse_key_value(key, value, op) {
                let range = key_match.start() - removed..value_match.end() - removed;
                search_text.replace_range(range, "");
                removed += value_match.end() - key_match.start();
            }
        }

        fn adjust_search_text(search_text: &mut String) {
            // Index of the last non-whitespace char
            let mut trunc_idx = search_text
                .char_indices()
                .rev()
                .find_map(|(i, c)| (!c.is_whitespace()).then(|| i + c.len_utf8()))
                .unwrap_or(0);

            // Index of the first non-whitespace char
            let start = search_text
                .char_indices()
                .find_map(|(i, c)| (!c.is_whitespace()).then_some(i))
                .filter(|&i| i > 0);

            // If there is whitespace at the front, rotate to the left until
            // the string starts with the first non-whitespace char
            if let Some(shift) = start {
                // SAFETY: The shift is given by .char_indices which is a valid idx
                unsafe { search_text.as_bytes_mut() }.rotate_left(shift);
                trunc_idx -= shift;
            }

            // Truncate the whitespace
            if trunc_idx < search_text.len() {
                search_text.truncate(trunc_idx);
            }

            search_text.make_ascii_lowercase();
        }

        adjust_search_text(&mut search_text);

        Self { inner, search_text }
    }

    pub fn has_search_terms(&self) -> bool {
        !self.search_text.is_empty()
    }

    pub fn search_terms(&self) -> impl Iterator<Item = &str> {
        self.search_text.split_whitespace()
    }

    pub fn display(&self, content: &mut String) {
        self.inner.display(content);

        if self.has_search_terms() {
            separate_content(content);

            if self.inner.any_field() {
                content.push_str("`Remaining query: ");
            } else {
                content.push_str("`Query: ");
            }

            content.push_str(&self.search_text);
            content.push('`');
        }
    }
}

impl<F> Deref for FilterCriteria<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
