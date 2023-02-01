use std::fmt::{Display, Formatter, Result as FmtResult};

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

                    write!(f, "{}", rev % 1000)?;

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

                    write!(f, "{}", rev % 1000)?;

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

                    write!(f, "{}", rev % 1000)?;

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
}
