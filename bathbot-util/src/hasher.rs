pub use int_hasher::IntHash;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct IntHasher;

#[cfg(debug_assertions)]
mod int_hasher {
    use std::hash::{BuildHasher, Hasher};

    #[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
    pub struct IntHash(Option<u64>);

    #[rustfmt::skip]
    impl Hasher for IntHash {
        fn write(&mut self, _: &[u8])     { panic!("don't use this"); }
        fn write_u128(&mut self, _: u128) { panic!("don't use this"); }
        fn write_i128(&mut self, _: i128) { panic!("don't use this"); }

        fn write_u8(&mut self, n: u8)       { assert!(self.0.replace(u64::from(n)).is_none(), "already hashed"); }
        fn write_u16(&mut self, n: u16)     { assert!(self.0.replace(u64::from(n)).is_none(), "already hashed"); }
        fn write_u32(&mut self, n: u32)     { assert!(self.0.replace(u64::from(n)).is_none(), "already hashed"); }
        fn write_u64(&mut self, n: u64)     { assert!(self.0.replace(n).is_none(), "already hashed");            }
        fn write_usize(&mut self, n: usize) { assert!(self.0.replace(n as u64).is_none(), "already hashed");     }

        fn write_i8(&mut self, n: i8)       { assert!(self.0.replace(n as u64).is_none(), "already hashed"); }
        fn write_i16(&mut self, n: i16)     { assert!(self.0.replace(n as u64).is_none(), "already hashed"); }
        fn write_i32(&mut self, n: i32)     { assert!(self.0.replace(n as u64).is_none(), "already hashed"); }
        fn write_i64(&mut self, n: i64)     { assert!(self.0.replace(n as u64).is_none(), "already hashed"); }
        fn write_isize(&mut self, n: isize) { assert!(self.0.replace(n as u64).is_none(), "already hashed"); }

        fn finish(&self) -> u64 { self.0.expect("missing hash") }
    }

    impl BuildHasher for super::IntHasher {
        type Hasher = IntHash;

        #[inline]
        fn build_hasher(&self) -> Self::Hasher {
            IntHash(None)
        }
    }
}

#[cfg(not(debug_assertions))]
mod int_hasher {
    use std::hash::{BuildHasher, Hasher};

    #[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
    pub struct IntHash(u64);

    #[rustfmt::skip]
    impl Hasher for IntHash {
        fn write(&mut self, _: &[u8])     { panic!("don't use this"); }
        fn write_u128(&mut self, _: u128) { panic!("don't use this"); }
        fn write_i128(&mut self, _: i128) { panic!("don't use this"); }
    
        #[inline] fn write_u8(&mut self, n: u8)       { self.0 = u64::from(n) }
        #[inline] fn write_u16(&mut self, n: u16)     { self.0 = u64::from(n) }
        #[inline] fn write_u32(&mut self, n: u32)     { self.0 = u64::from(n) }
        #[inline] fn write_u64(&mut self, n: u64)     { self.0 = n            }
        #[inline] fn write_usize(&mut self, n: usize) { self.0 = n as u64     }
    
        #[inline] fn write_i8(&mut self, n: i8)       { self.0 = n as u64 }
        #[inline] fn write_i16(&mut self, n: i16)     { self.0 = n as u64 }
        #[inline] fn write_i32(&mut self, n: i32)     { self.0 = n as u64 }
        #[inline] fn write_i64(&mut self, n: i64)     { self.0 = n as u64 }
        #[inline] fn write_isize(&mut self, n: isize) { self.0 = n as u64 }
    
        #[inline] fn finish(&self) -> u64 { self.0 }
    }

    impl BuildHasher for super::IntHasher {
        type Hasher = IntHash;

        #[inline]
        fn build_hasher(&self) -> Self::Hasher {
            IntHash(0)
        }
    }
}
