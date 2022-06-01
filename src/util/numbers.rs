use std::fmt;

/// Round with two decimal positions
pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
}

pub fn with_comma_float(n: f32) -> FormatF32 {
    FormatF32(n)
}

pub struct FormatF32(f32);

impl fmt::Display for FormatF32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = if self.0 < 0.0 {
            f.write_str("-")?;

            -self.0
        } else {
            self.0
        };

        let mut int = n.trunc() as i64;
        let mut rev = 0;
        let mut triples = 0;

        while int > 0 {
            rev = rev * 1000 + int % 1000;
            int /= 1000;
            triples += 1;
        }

        write!(f, "{}", rev % 1000)?;

        for _ in 0..triples - 1 {
            rev /= 1000;
            write!(f, ",{:0>3}", rev % 1000)?;
        }

        let mut dec = (100.0 * n.fract()).round() as u32;

        if dec > 0 {
            if dec % 10 == 0 {
                dec /= 10;
            }

            write!(f, ".{dec}")?;
        }

        Ok(())
    }
}

pub fn with_comma_int<T: Int>(n: T) -> FormatInt {
    FormatInt(n.into_i64())
}

pub struct FormatInt(i64);

impl fmt::Display for FormatInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut n = if self.0 < 0 {
            f.write_str("-")?;

            -self.0
        } else {
            self.0
        };

        let mut rev = 0;
        let mut triples = 0;

        while n > 0 {
            rev = rev * 1000 + n % 1000;
            n /= 1000;
            triples += 1;
        }

        write!(f, "{}", rev % 1000)?;

        for _ in 0..triples - 1 {
            rev /= 1000;
            write!(f, ",{:0>3}", rev % 1000)?;
        }

        Ok(())
    }
}

pub trait Int {
    fn into_i64(self) -> i64;
}

macro_rules! into_int {
    ($ty:ty) => {
        impl Int for $ty {
            fn into_i64(self) -> i64 {
                self as i64
            }
        }
    };
}

into_int!(u8);
into_int!(u16);
into_int!(u32);
into_int!(u64);
into_int!(usize);

into_int!(i8);
into_int!(i16);
into_int!(i32);
into_int!(i64);
into_int!(isize);

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
    fn test_with_comma_int() {
        assert_eq!(
            with_comma_int(31_415_926_u32).to_string(),
            "31,415,926".to_owned()
        );
    }

    #[test]
    fn test_with_comma_f32() {
        assert_eq!(
            with_comma_float(31_925.53).to_string(),
            "31,925.53".to_owned()
        );
    }
}
