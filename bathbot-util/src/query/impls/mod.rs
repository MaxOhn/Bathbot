use std::fmt::{Debug, Write};

pub use self::{bookmark::BookmarkCriteria, regular::RegularCriteria, top::TopCriteria};
use super::{
    operator::Operator,
    optional::{OptionalRange, OptionalText},
    separate_content,
};

mod bookmark;
mod regular;
mod top;

fn try_update_len(length: &mut OptionalRange<f32>, op: Operator, value: &str) -> bool {
    let Ok(len) = value.trim_end_matches(['m', 's', 'h']).parse::<f32>() else {
        return false;
    };

    let scale = if value.ends_with("ms") {
        1.0 / 1000.0
    } else if value.ends_with('s') {
        1.0
    } else if value.ends_with('m') {
        60.0
    } else if value.ends_with('h') {
        3_600.0
    } else {
        1.0
    };

    length.try_update_value(op, len * scale, scale / 2.0)
}

fn display_range<T>(content: &mut String, name: &str, range: &OptionalRange<T>)
where
    OptionalRange<T>: Debug,
{
    if !range.is_empty() {
        separate_content(content);
        let _ = write!(content, "`{name}: {range:?}`");
    }
}

fn display_text(content: &mut String, name: &str, text: &OptionalText<'_>) {
    if !text.is_empty() {
        separate_content(content);
        let _ = write!(content, "`{name}: {text:?}`");
    }
}
