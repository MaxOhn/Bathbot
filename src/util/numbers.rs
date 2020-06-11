use num_format::{Locale, WriteFormatted};
use std::char;

pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
}

pub fn round_precision(n: f32, precision: i32) -> f32 {
    let adj = 10.0_f32.powi(precision);
    (adj * n).round() / adj
}

pub fn with_comma(n: f32) -> String {
    let dec = (100.0 * n.fract()).round() / 100.0;
    let int = n.trunc();
    assert!(int >= 0.0);
    let mut int = int as u32;
    let mut writer = String::new();
    loop {
        for _ in 0..3 {
            writer.push(char::from_digit(int % 10, 10).unwrap());
            int /= 10;
            if int == 0 {
                break;
            }
        }
        if int > 0 {
            writer.push(',');
        } else {
            break;
        }
    }
    let mut writer: String = writer.chars().rev().collect();
    if dec > 0.0 {
        let d = dec.to_string();
        writer.push_str(&d[1..d.len()])
    }
    writer
}

pub fn with_comma_u64(n: u64) -> String {
    let mut writer = String::new();
    writer.write_formatted(&n, &Locale::en).unwrap();
    writer
}

pub fn round_and_comma(n: f32) -> String {
    with_comma(round(n))
}

pub fn div_euclid(group: usize, total: usize) -> usize {
    if total % group == 0 && total > 0 {
        total / group
    } else {
        total.div_euclid(group) + 1
    }
}

pub fn last_multiple(per_page: usize, total: usize) -> usize {
    if per_page <= total && total % per_page == 0 {
        total - per_page
    } else {
        total - total % per_page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        assert_eq!(round(3.1415), 3.14);
    }

    #[test]
    fn test_with_comma_u64() {
        assert_eq!(with_comma_u64(31_415_926), "31,415,926".to_owned());
    }

    #[test]
    fn test_with_comma_f32() {
        assert_eq!(with_comma(31_925.53), "31,925.53".to_owned());
    }

    #[test]
    fn test_round_and_comma() {
        assert_eq!(round_and_comma(31926.535897), "31,926.54".to_owned());
    }
}
