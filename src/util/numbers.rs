use num_format::{Locale, WriteFormatted};
use std::char;

pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
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

pub fn with_comma_u32(n: u32) -> String {
    let mut writer = String::new();
    writer.write_formatted(&n, &Locale::en).unwrap();
    writer
}

pub fn round_and_comma(n: f32) -> String {
    with_comma(round(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        assert_eq!(round(3.1415), 3.14);
    }

    #[test]
    fn test_with_comma_u32() {
        assert_eq!(with_comma_u32(31415926), "31,415,926".to_owned());
    }

    #[test]
    fn test_with_comma() {
        assert_eq!(with_comma(31415926.53), "31,415,926.53".to_owned());
    }

    #[test]
    fn test_round_and_comma() {
        assert_eq!(round_and_comma(31415926.535897), "31,415,926.54".to_owned());
    }
}
