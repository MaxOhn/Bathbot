mod filter;
mod operator;
mod optional;
mod regular_criteria;
mod scores_criteria;
mod searchable;

pub use self::{
    filter::*, regular_criteria::RegularCriteria, scores_criteria::ScoresCriteria, searchable::*,
};
use self::{operator::Operator, optional::OptionalRange};

fn try_update_len(length: &mut OptionalRange<f32>, op: Operator, value: &str) -> bool {
    let Ok(len) = value.trim_end_matches(&['m', 's', 'h']).parse::<f32>() else {
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
