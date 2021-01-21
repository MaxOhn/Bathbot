use super::constants::DATE_FORMAT;
use crate::BotResult;

use chrono::{offset::TimeZone, DateTime, Datelike, Utc};

#[inline]
pub fn date_to_string(date: &DateTime<Utc>) -> String {
    date.format(DATE_FORMAT).to_string()
}

#[allow(dead_code)]
#[inline]
pub fn string_to_date(date: String) -> BotResult<DateTime<Utc>> {
    Ok(Utc.datetime_from_str(&date, DATE_FORMAT)?)
}

#[inline]
pub fn sec_to_minsec(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

// thx saki :)
pub fn how_long_ago(date: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff_sec = now.timestamp() - date.timestamp();
    assert!(diff_sec >= 0);
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
                (12 * (now.year() - date.year()) as u32 + now.month() - date.month()) as i64;
            if diff_month < 1 {
                (diff_sec / one_week, "week")
            } else if diff_month < 12 {
                (diff_month, "month")
            } else {
                let mut years = diff_month / 12;
                if diff_month % 12 > 9 {
                    years += 1
                }
                (years, "year")
            }
        }
    };

    format!(
        "{amount} {unit}{plural} ago",
        amount = amount,
        unit = unit,
        plural = if amount == 1 { "" } else { "s" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec_to_minsec() {
        assert_eq!(sec_to_minsec(92), String::from("1:32"));
        assert_eq!(sec_to_minsec(3605), String::from("60:05"));
    }
}
