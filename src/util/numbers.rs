use std::fmt::Write;

pub fn _clamp_map(in_min: f32, in_max: f32, out_min: f32, out_max: f32, input: f32) -> f32 {
    out_min + ((out_max - out_min) / (in_max - in_min)) * (input - in_min)
}

/// Round with two decimal positions
pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
}

pub fn with_comma(n: f32) -> String {
    let mut int = n.trunc() as i64;
    assert!(int >= 0, "cannot round negative f32");
    let size = match int {
        _ if int < 1000 => 6,
        _ if int < 1_000_000 => 10,
        _ => 14,
    };
    let mut writer = String::with_capacity(size);
    let mut rev = 0;
    let mut triples = 0;
    while int > 0 {
        rev = rev * 1000 + int % 1000;
        int /= 1000;
        triples += 1;
    }
    let _ = write!(writer, "{}", rev % 1000);
    rev /= 1000;
    for _ in 0..triples - 1 {
        let _ = write!(writer, ",{:0>3}", rev % 1000);
        rev /= 1000;
    }
    let mut dec = (100.0 * n.fract()).round() as u32;
    if dec > 0 {
        if dec % 10 == 0 {
            dec /= 10;
        }
        let _ = write!(writer, ".{}", dec);
    }
    writer
}

pub fn with_comma_u64(mut n: u64) -> String {
    let size = match n {
        _ if n < 1000 => 3,
        _ if n < 1_000_000 => 7,
        _ if n < 1_000_000_000 => 11,
        _ => 15,
    };
    let mut writer = String::with_capacity(size);
    let mut rev = 0;
    let mut triples = 0;
    while n > 0 {
        rev = rev * 1000 + n % 1000;
        n /= 1000;
        triples += 1;
    }
    let _ = write!(writer, "{}", rev % 1000);
    rev /= 1000;
    for _ in 0..triples - 1 {
        let _ = write!(writer, ",{:0>3}", rev % 1000);
        rev /= 1000;
    }
    writer
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
        let v1 = 3.1615;
        let v2 = 3.16;
        if round(v1) - v2 > std::f32::EPSILON {
            panic!("[test_round] round({})={} != {}", v1, round(v1), v2);
        }
    }

    #[test]
    fn test_with_comma_u64() {
        assert_eq!(with_comma_u64(31_415_926_u64), "31,415,926".to_owned());
    }

    #[test]
    fn test_with_comma_f32() {
        assert_eq!(with_comma(31_925.53), "31,925.53".to_owned());
    }
}
