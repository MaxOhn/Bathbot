use std::fmt::{Display, Formatter, Result as FmtResult};

use time::{
    format_description::{
        modifier::{Day, Hour, Minute, Month, OffsetHour, OffsetMinute, Second, Year},
        Component, FormatItem,
    },
    OffsetDateTime,
};

pub struct SecToMinSec {
    secs: u32,
    pad_secs: bool,
}

impl SecToMinSec {
    pub fn new(secs: u32) -> Self {
        Self {
            secs,
            pad_secs: false,
        }
    }

    pub fn pad_secs(self) -> Self {
        Self {
            pad_secs: true,
            ..self
        }
    }
}

impl Display for SecToMinSec {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.pad_secs {
            write!(f, "{:02}", self.secs / 60)?;
        } else {
            write!(f, "{}", self.secs / 60)?;
        }

        write!(f, ":{:02}", self.secs % 60)
    }
}

pub struct HowLongAgoText {
    secs: i64,
    year: i32,
    month: u32,
}

impl HowLongAgoText {
    pub fn new(datetime: &OffsetDateTime) -> Self {
        let date = datetime.date();

        Self {
            secs: datetime.unix_timestamp(),
            year: date.year(),
            month: date.month() as u32,
        }
    }
}

// thx saki :)
impl Display for HowLongAgoText {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let now = OffsetDateTime::now_utc();
        let diff_sec = now.unix_timestamp() - self.secs;
        debug_assert!(diff_sec >= 0);

        let one_day = 24 * 3600;
        let one_week = 7 * one_day;

        let (amount, unit) = {
            if diff_sec < 60 {
                (diff_sec, "second")
            } else if diff_sec < 3600 {
                (diff_sec / 60, "minute")
            } else if diff_sec < one_day {
                (diff_sec / 3600, "hour")
            } else if diff_sec < one_week {
                (diff_sec / one_day, "day")
            } else if diff_sec < 4 * one_week {
                (diff_sec / one_week, "week")
            } else {
                let diff_month =
                    (12 * (now.year() - self.year) as u32 + now.month() as u32 - self.month) as i64;

                if diff_month < 1 {
                    (diff_sec / one_week, "week")
                } else if diff_month < 12 {
                    (diff_month, "month")
                } else {
                    let years = diff_month / 12 + (diff_month % 12 > 9) as i64;

                    (years, "year")
                }
            }
        };

        write!(
            f,
            "{amount} {unit}{plural} ago",
            plural = if amount == 1 { "" } else { "s" }
        )
    }
}

/// Instead of writing the whole string like `HowLongAgoText`,
/// this just writes discord's syntax for dynamic timestamps and lets
/// discord handle the rest.
///
/// Note: Doesn't work in embed footers
#[derive(Copy, Clone)]
pub struct HowLongAgoDynamic {
    secs: i64,
}

impl HowLongAgoDynamic {
    pub fn new(datetime: &OffsetDateTime) -> Self {
        Self {
            secs: datetime.unix_timestamp(),
        }
    }
}

impl Display for HowLongAgoDynamic {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // https://discord.com/developers/docs/reference#message-formatting-timestamp-styles
        write!(f, "<t:{}:R>", self.secs)
    }
}

pub const DATE_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Component(Component::Year(Year::default())),
    FormatItem::Literal(b"-"),
    FormatItem::Component(Component::Month(Month::default())),
    FormatItem::Literal(b"-"),
    FormatItem::Component(Component::Day(Day::default())),
];

pub const TIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Component(Component::Hour(<Hour>::default())),
    FormatItem::Literal(b":"),
    FormatItem::Component(Component::Minute(<Minute>::default())),
    FormatItem::Literal(b":"),
    FormatItem::Component(Component::Second(<Second>::default())),
];

pub const SHORT_TIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Component(Component::Hour(<Hour>::default())),
    FormatItem::Literal(b":"),
    FormatItem::Component(Component::Minute(<Minute>::default())),
];

pub const OFFSET_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Component(Component::OffsetHour(OffsetHour::default())),
    FormatItem::Literal(b":"),
    FormatItem::Component(Component::OffsetMinute(OffsetMinute::default())),
];

pub const NAIVE_DATETIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATE_FORMAT),
    FormatItem::Literal(b" "),
    FormatItem::Compound(TIME_FORMAT),
];

pub const SHORT_NAIVE_DATETIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATE_FORMAT),
    FormatItem::Literal(b" "),
    FormatItem::Compound(SHORT_TIME_FORMAT),
];

pub const DATETIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATE_FORMAT),
    FormatItem::Literal(b"T"),
    FormatItem::Compound(TIME_FORMAT),
];

pub const DATETIME_Z_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATETIME_FORMAT),
    FormatItem::Literal(b"Z"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_to_minsec() {
        assert_eq!(SecToMinSec::new(92).to_string(), String::from("1:32"));
        assert_eq!(SecToMinSec::new(3605).to_string(), String::from("60:05"));
    }
}
