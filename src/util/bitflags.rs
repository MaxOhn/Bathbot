use std::fmt::{Display, Write};

use crate::{bg_game::MapsetTags, commands::fun::Effects};

pub struct IntoIter<F> {
    flags: F,
    shift: usize,
}

macro_rules! bitflag_impls {
    ($ty:ident, $size:literal) => {
        impl $ty {
            pub fn join(self, separator: impl Display) -> String {
                let mut iter = self.into_iter();

                let first_flag = match iter.next() {
                    Some(first_flag) => first_flag,
                    None => return "None".to_owned(),
                };

                let size = self.bits().count_ones() as usize;
                let mut result = String::with_capacity(size * 6);
                let _ = write!(result, "{first_flag:?}");

                for element in iter {
                    let _ = write!(result, "{separator}{element:?}");
                }

                result
            }
        }

        impl Iterator for IntoIter<$ty> {
            type Item = $ty;

            fn next(&mut self) -> Option<Self::Item> {
                if self.flags.is_empty() {
                    None
                } else {
                    loop {
                        if self.shift == $size {
                            return None;
                        }

                        let bit = 1 << self.shift;
                        self.shift += 1;

                        if self.flags.bits() & bit != 0 {
                            return $ty::from_bits(bit);
                        }
                    }
                }
            }
        }

        impl IntoIterator for $ty {
            type Item = $ty;
            type IntoIter = IntoIter<$ty>;

            fn into_iter(self) -> IntoIter<$ty> {
                IntoIter {
                    flags: self,
                    shift: 0,
                }
            }
        }
    };
}

bitflag_impls!(MapsetTags, 32);
bitflag_impls!(Effects, 8);
