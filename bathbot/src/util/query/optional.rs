use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    ops::{Add, Sub},
    str::FromStr,
    time::Duration,
};

use bathbot_util::{datetime::DATE_FORMAT, CowUtils};
use time::Date;

use super::operator::Operator;

#[derive(Default)]
pub struct OptionalText<'q> {
    search_term: Cow<'q, str>,
}

impl Debug for OptionalText<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.search_term.is_empty() {
            f.write_str("<none>")
        } else {
            f.write_str(self.search_term.as_ref())
        }
    }
}

impl<'q> OptionalText<'q> {
    pub fn is_empty(&self) -> bool {
        self.search_term.is_empty()
    }

    pub fn matches(&self, value: &str) -> bool {
        self.is_empty() || self.search_term == value.cow_to_ascii_lowercase()
    }

    pub fn try_update(&mut self, op: Operator, value: Cow<'q, str>) -> bool {
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

pub struct OptionalRange<T> {
    min: Option<T>,
    max: Option<T>,

    is_lower_inclusive: bool,
    is_upper_inclusive: bool,
}

impl<T> Default for OptionalRange<T> {
    #[inline]
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            is_lower_inclusive: false,
            is_upper_inclusive: false,
        }
    }
}

impl<N> OptionalRange<N> {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn try_update<T>(&mut self, op: Operator, value: &str, tolerance: T) -> bool
    where
        N: Copy + FromStr + Add<T, Output = N> + Sub<T, Output = N>,
        T: Copy,
    {
        value
            .parse()
            .is_ok_and(|value| self.try_update_value(op, value, tolerance))
    }

    pub fn try_update_value<T>(&mut self, op: Operator, value: N, tolerance: T) -> bool
    where
        N: Copy + Add<T, Output = N> + Sub<T, Output = N>,
        T: Copy,
    {
        match op {
            Operator::Equal => {
                self.min = Some(value - tolerance);
                self.max = Some(value + tolerance);
                self.is_lower_inclusive = true;
                self.is_upper_inclusive = true;
            }
            Operator::Less => self.max = Some(value - tolerance),
            Operator::LessOrEqual => {
                self.max = Some(value + tolerance);
                self.is_upper_inclusive = true;
            }
            Operator::Greater => self.min = Some(value + tolerance),
            Operator::GreaterOrEqual => {
                self.min = Some(value - tolerance);
                self.is_lower_inclusive = true;
            }
        }

        true
    }
}

impl OptionalRange<Date> {
    pub fn try_update_date(&mut self, op: Operator, value: &str) -> bool {
        Date::parse(value, &DATE_FORMAT)
            .is_ok_and(|date| self.try_update_value(op, date, Duration::ZERO))
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
                Some(Ordering::Equal) => return self.is_upper_inclusive,
                Some(Ordering::Greater) | None => return false,
            }
        }

        true
    }
}

impl Debug for OptionalRange<f32> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        fmt_float(self, f)
    }
}

impl Debug for OptionalRange<u32> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        fmt_eq(self, f)
    }
}

impl Debug for OptionalRange<Date> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        fmt_eq(self, f)
    }
}

fn fmt_float(optional: &OptionalRange<f32>, f: &mut Formatter<'_>) -> FmtResult {
    if optional.min.is_none()
        && optional.max.is_none()
        && !optional.is_lower_inclusive
        && !optional.is_upper_inclusive
    {
        return f.write_str("..");
    }

    if optional.is_lower_inclusive {
        f.write_str("[")?;
    } else {
        f.write_str("(")?;
    }

    if let Some(ref min) = optional.min {
        write!(f, "{min}")?;
    }

    if optional.min.is_some() && optional.max.is_some() {
        f.write_str(",")?;
    } else {
        f.write_str("..")?;
    }

    if let Some(ref max) = optional.max {
        write!(f, "{max}")?;
    }

    if optional.is_upper_inclusive {
        f.write_str("]")?;
    } else {
        f.write_str(")")?;
    }

    Ok(())
}

fn fmt_eq<T: Copy + Display + Eq>(optional: &OptionalRange<T>, f: &mut Formatter<'_>) -> FmtResult {
    if optional.min.is_none()
        && optional.max.is_none()
        && !optional.is_lower_inclusive
        && !optional.is_upper_inclusive
    {
        return f.write_str("..");
    } else if let Some(value) = optional
        .min
        .zip(optional.max)
        .filter(|(min, max)| min == max)
        .map(|(min, _)| min)
    {
        return write!(f, "{value}");
    }

    if optional.is_lower_inclusive {
        f.write_str("[")?;
    } else {
        f.write_str("(")?;
    }

    if let Some(ref min) = optional.min {
        write!(f, "{min}")?;
    }

    if optional.min.is_some() && optional.max.is_some() {
        f.write_str(",")?;
    } else {
        f.write_str("..")?;
    }

    if let Some(ref max) = optional.max {
        write!(f, "{max}")?;
    }

    if optional.is_upper_inclusive {
        f.write_str("]")?;
    } else {
        f.write_str(")")?;
    }

    Ok(())
}
