use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    ops::{AddAssign, Div},
};

/// Round with two decimal positions
pub fn round(n: f32) -> f32 {
    (100.0 * n).round() / 100.0
}

pub struct WithComma<N> {
    num: N,
}

impl<N> WithComma<N> {
    pub fn new(num: N) -> Self {
        Self { num }
    }
}

macro_rules! impl_with_comma {
    (@FLOAT: $( $ty:ty ),* ) => {
        $(
            impl Display for WithComma<$ty> {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                    let n = if self.num < 0.0 {
                        f.write_str("-")?;

                        -self.num
                    } else {
                        self.num
                    };

                    let mut int = n.trunc() as i64;
                    let mut rev = 0;
                    let mut triples = 0;

                    while int > 0 {
                        rev = rev * 1000 + int % 1000;
                        int /= 1000;
                        triples += 1;
                    }

                    Display::fmt(&(rev % 1000), f)?;

                    for _ in 0..triples - 1 {
                        rev /= 1000;
                        write!(f, ",{:0>3}", rev % 1000)?;
                    }

                    let dec = (100.0 * n.fract()).round() as u32;

                    if dec > 0 {
                        f.write_str(".")?;

                        if dec < 10 {
                            write!(f, "0{dec}")?;
                        } else if dec == 100 {
                            f.write_str("99")?;
                        } else {
                            write!(f, "{dec}")?;
                        }
                    }

                    Ok(())
                }
            }
        )*
    };
    (@INT: $( $ty:ident $( > $cutoff:literal -> $backup:ident )? ),* ) => {
        $(
            impl Display for WithComma<$ty> {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                    $(
                        // Preventing potential overflows
                        if self.num.abs() > $cutoff {
                            return WithComma::new(self.num as $backup).fmt(f);
                        }
                    )?

                    let mut n = if self.num < 0 {
                        f.write_str("-")?;

                        -self.num
                    } else {
                        self.num
                    };

                    let mut rev = 0;
                    let mut triples = 0;

                    while n > 0 {
                        rev = rev * 1000 + n % 1000;
                        n /= 1000;
                        triples += 1;
                    }

                    Display::fmt(&(rev % 1000), f)?;

                    for _ in 0..triples - 1 {
                        rev /= 1000;
                        write!(f, ",{:0>3}", rev % 1000)?;
                    }

                    Ok(())
                }
            }
        )*
    };
    (@UINT: $( $ty:ident $( > $cutoff:literal -> $backup:ident )? ),* ) => {
        $(
            impl Display for WithComma<$ty> {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                    $(
                        // Preventing potential overflows
                        if self.num > $cutoff {
                            return WithComma::new(self.num as $backup).fmt(f);
                        }
                    )?

                    let mut n = self.num;
                    let mut rev = 0;
                    let mut triples = 0;

                    while n > 0 {
                        rev = rev * 1000 + n % 1000;
                        n /= 1000;
                        triples += 1;
                    }

                    Display::fmt(&(rev % 1000), f)?;

                    for _ in 0..triples - 1 {
                        rev /= 1000;
                        write!(f, ",{:0>3}", rev % 1000)?;
                    }

                    Ok(())
                }
            }
        )*
    };
}

impl_with_comma!(@FLOAT: f32, f64);
impl_with_comma!(@INT: i16 > 1032 -> i32, i32 > 1_000_000_002 -> i64, i64, isize);
impl_with_comma!(@UINT: u16 > 1065 -> u32, u32 > 1_000_000_004 -> u64, u64, usize);

pub fn last_multiple(per_page: usize, total: usize) -> usize {
    if per_page <= total && total % per_page == 0 {
        total - per_page
    } else {
        total - total % per_page
    }
}

pub trait Number: AddAssign + Copy + Div<Output = Self> + PartialOrd {
    fn zero() -> Self;
    fn max() -> Self;
    fn min() -> Self;
    fn inc(&mut self);
}

macro_rules! impl_number {
    ( $( $ty:ident: $one:literal ),* ) => {
        $(
           impl Number for $ty {
                fn zero() -> Self { $ty::default() }
                fn max() -> Self { $ty::MAX }
                fn min() -> Self { $ty::MIN }
                fn inc(&mut self) { *self += $one }
            }
        )*
    }
}

impl_number!(u32: 1, f32: 1.0, f64: 1.0);

pub struct MinMaxAvg<N> {
    min: N,
    max: N,
    sum: N,
    len: N,
}

impl<N: Number> Default for MinMaxAvg<N> {
    fn default() -> Self {
        Self {
            min: N::max(),
            max: N::min(),
            sum: N::zero(),
            len: N::zero(),
        }
    }
}

impl<N: Number> MinMaxAvg<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, n: N) {
        if self.min > n {
            self.min = n;
        }

        if self.max < n {
            self.max = n;
        }

        self.sum += n;
        self.len.inc();
    }

    pub fn min(&self) -> N {
        self.min
    }

    pub fn max(&self) -> N {
        self.max
    }

    pub fn avg(&self) -> N {
        self.sum / self.len
    }
}

pub trait AsFloat {
    fn into_f32(self) -> f32;
    fn into_f64(self) -> f64;
}

macro_rules! impl_as_float {
    ( $( $ty:ident ),* ) => {
        $(
            impl AsFloat for $ty {
                #[inline]
                fn into_f32(self) -> f32 {
                    self as f32
                }

                #[inline]
                fn into_f64(self) -> f64 {
                    self as f64
                }
            }
        )*
    }
}

impl_as_float!(u32);

impl<N: Number + AsFloat> MinMaxAvg<N> {
    pub fn avg_float(&self) -> f32 {
        self.sum.into_f32() / self.len.into_f32()
    }
}

impl From<MinMaxAvg<f32>> for MinMaxAvg<u32> {
    fn from(other: MinMaxAvg<f32>) -> Self {
        Self {
            min: other.min as u32,
            max: other.max as u32,
            sum: other.sum as u32,
            len: other.len as u32,
        }
    }
}

pub struct AbbreviatedScore {
    score: u64,
}

impl AbbreviatedScore {
    pub fn new(score: u64) -> Self {
        Self { score }
    }
}

impl Display for AbbreviatedScore {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let score = self.score as f64;
        if score >= 1_000_000_000_000.0 {
            write!(f, "{:.2}tn", score / 1_000_000_000_000.0)
        } else if score >= 1_000_000_000.0 {
            write!(f, "{:.2}bn", score / 1_000_000_000.0)
        } else if score >= 1_000_000.0 {
            write!(f, "{:.2}m", score / 1_000_000.0)
        } else {
            Display::fmt(&WithComma::new(self.score), f)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        let v1 = 3.1615;
        let v2 = 3.16;

        if round(v1) - v2 > f32::EPSILON {
            panic!("[test_round] round({})={} != {}", v1, round(v1), v2);
        }
    }

    #[test]
    fn test_with_comma_int() {
        assert_eq!(
            WithComma::new(31_415_926_u32).to_string(),
            "31,415,926".to_owned()
        );
    }

    #[test]
    fn test_with_comma_f32() {
        assert_eq!(
            WithComma::new(31_925.53_f32).to_string(),
            "31,925.53".to_owned()
        );
    }

    #[test]
    fn test_abbreviated_score() {
        assert_eq!(
            AbbreviatedScore::new(1_372_111_816_859_u64).to_string(),
            "1.37tn".to_owned()
        );

        assert_eq!(
            AbbreviatedScore::new(893_135_435_096_u64).to_string(),
            "893.14bn".to_owned()
        );

        assert_eq!(
            AbbreviatedScore::new(136_976_283_u64).to_string(),
            "136.98m".to_owned()
        );

        assert_eq!(AbbreviatedScore::new(727_u64).to_string(), "727".to_owned());
    }
}
