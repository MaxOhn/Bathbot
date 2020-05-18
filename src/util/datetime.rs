use super::Error;
use crate::util::globals::DATE_FORMAT;
use chrono::{offset::TimeZone, DateTime, Datelike, Utc};

pub fn date_to_string(date: &DateTime<Utc>) -> String {
    date.format(DATE_FORMAT).to_string()
}

#[allow(unused)]
pub fn string_to_date(date: String) -> Result<DateTime<Utc>, Error> {
    Utc.datetime_from_str(&date, "%F %T")
        .map_err(Error::ParseChrono)
}

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
        } else {
            let diff_month =
                (12 * (now.year() - date.year()) as u32 + now.month() - date.month()) as i64;
            if diff_month < 1 {
                (diff_sec / one_week, "week")
            } else if diff_month < 12 {
                (diff_month, "month")
            } else {
                (diff_month / 12, "year")
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

pub fn _how_long_ago(date: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff_sec = now.timestamp() - date.timestamp();
    assert!(diff_sec >= 0);
    let (amount, unit) = {
        let diff_min = diff_sec / 60;
        if diff_min < 1 {
            (diff_sec, "second")
        } else {
            let diff_hour = diff_sec / 3600; // 60*60
            if diff_hour < 1 {
                (diff_min, "minute")
            } else {
                let diff_day = diff_sec / 86_400; // 3600*24
                if diff_day < 1 {
                    (diff_hour, "hour")
                } else {
                    let diff_week = diff_sec / 604_800; // 86_400*7
                    if diff_week < 1 {
                        (diff_day, "day")
                    } else {
                        let mut diff_month = diff_sec / 2_628_000; // 86_400*30.416667
                        if diff_month < 1 {
                            (diff_week, "week")
                        } else {
                            let mut diff_year = diff_sec / 31_536_000; // 86_400*365
                            if diff_year < 1 {
                                if (diff_day as f64 - diff_month as f64 * 30.416_667) > 20.0 {
                                    diff_month += 1;
                                }
                                (diff_month, "month")
                            } else {
                                if (diff_month - diff_year * 12) > 6 {
                                    diff_year += 1;
                                }
                                (diff_year, "year")
                            }
                        }
                    }
                }
            }
        }
    };
    format!(
        "{} {}{} ago",
        amount,
        unit,
        if amount == 1 { "" } else { "s" }
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
