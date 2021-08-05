use std::fmt;

/// Round with two decimal positions
pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
}

pub struct FormatF32(f32);

impl fmt::Display for FormatF32 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut int = self.0.trunc() as i64;
        debug_assert!(int >= 0, "cannot round negative f32");

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

        let mut dec = (100.0 * self.0.fract()).round() as u32;

        if dec > 0 {
            if dec % 10 == 0 {
                dec /= 10;
            }

            write!(f, ".{}", dec)?;
        }

        Ok(())
    }
}

pub fn with_comma_float(n: f32) -> FormatF32 {
    FormatF32(n)
}

pub struct FormatUint(u64);

impl fmt::Display for FormatUint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut n = self.0;

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

pub trait Uint {
    fn into_u64(self) -> u64;
}

macro_rules! into_uint {
    ($ty:ty) => {
        impl Uint for $ty {
            fn into_u64(self) -> u64 {
                self as u64
            }
        }
    };
}

into_uint!(u8);
into_uint!(u16);
into_uint!(u32);
into_uint!(u64);
into_uint!(usize);

pub fn with_comma_uint<T: Uint>(n: T) -> FormatUint {
    FormatUint(n.into_u64())
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
        assert_eq!(
            with_comma_uint(31_415_926_u32).to_string(),
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
