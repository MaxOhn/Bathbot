use super::constants::DATE_FORMAT;
use crate::BotResult;

use chrono::{offset::TimeZone, DateTime, Utc};
use std::fmt;

pub fn date_to_string(date: &DateTime<Utc>) -> String {
    date.format(DATE_FORMAT).to_string()
}

#[allow(dead_code)]
pub fn string_to_date(date: String) -> BotResult<DateTime<Utc>> {
    Ok(Utc.datetime_from_str(&date, DATE_FORMAT)?)
}

pub fn sec_to_minsec(secs: u32) -> SecToMinSecFormatter {
    SecToMinSecFormatter { secs }
}

pub struct SecToMinSecFormatter {
    secs: u32,
}

impl fmt::Display for SecToMinSecFormatter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{:02}", self.secs / 60, self.secs % 60)
    }
}

pub fn how_long_ago(date: &DateTime<Utc>) -> HowLongAgoFormatter {
    HowLongAgoFormatter(date.timestamp())
}

pub struct HowLongAgoFormatter(i64);

impl fmt::Display for HowLongAgoFormatter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // https://discord.com/developers/docs/reference#message-formatting-timestamp-styles
        write!(f, "<t:{}:R>", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_to_minsec() {
        assert_eq!(sec_to_minsec(92).to_string(), String::from("1:32"));
        assert_eq!(sec_to_minsec(3605).to_string(), String::from("60:05"));
    }
}
