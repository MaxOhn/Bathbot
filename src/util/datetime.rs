use std::fmt;

use time::{
    format_description::{
        modifier::{Day, Hour, Minute, Month, OffsetHour, OffsetMinute, Second, Year},
        Component, FormatItem,
    },
    OffsetDateTime,
};

pub fn sec_to_minsec(secs: u32) -> SecToMinSecFormatter {
    SecToMinSecFormatter { secs }
}

pub struct SecToMinSecFormatter {
    secs: u32,
}

impl fmt::Display for SecToMinSecFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:02}", self.secs / 60, self.secs % 60)
    }
}

// thx saki :)
pub fn how_long_ago_text(date: &OffsetDateTime) -> HowLongAgoFormatterText<'_> {
    HowLongAgoFormatterText(date)
}

pub struct HowLongAgoFormatterText<'a>(&'a OffsetDateTime);

impl<'a> fmt::Display for HowLongAgoFormatterText<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let now = OffsetDateTime::now_utc();
        let diff_sec = now.unix_timestamp() - self.0.unix_timestamp();
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
                let diff_month = (12 * (now.year() - self.0.year()) as u32 + now.month() as u32
                    - self.0.month() as u32) as i64;

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

/// Instead of writing the whole string like `how_long_ago_text`,
/// this just writes discord's syntax for dynamic timestamps and lets
/// discord handle the rest.
///
/// Note: Doesn't work in embed footers
pub fn how_long_ago_dynamic(date: &OffsetDateTime) -> HowLongAgoFormatterDynamic {
    HowLongAgoFormatterDynamic(date.unix_timestamp())
}

#[derive(Copy, Clone)]
pub struct HowLongAgoFormatterDynamic(i64);

impl fmt::Display for HowLongAgoFormatterDynamic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // https://discord.com/developers/docs/reference#message-formatting-timestamp-styles
        write!(f, "<t:{}:R>", self.0)
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

pub const UTC_OFFSET_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Component(Component::OffsetHour(OffsetHour::default())),
    FormatItem::Literal(b":"),
    FormatItem::Component(Component::OffsetMinute(OffsetMinute::default())),
];

pub const DATETIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATE_FORMAT),
    FormatItem::Literal(b" "),
    FormatItem::Compound(TIME_FORMAT),
];

pub const OFFSET_DATETIME_FORMAT: &[FormatItem<'_>] = &[
    FormatItem::Compound(DATE_FORMAT),
    FormatItem::Literal(b"T"),
    FormatItem::Compound(TIME_FORMAT),
    FormatItem::Compound(UTC_OFFSET_FORMAT),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_to_minsec() {
        assert_eq!(sec_to_minsec(92).to_string(), String::from("1:32"));
        assert_eq!(sec_to_minsec(3605).to_string(), String::from("60:05"));
    }
}
