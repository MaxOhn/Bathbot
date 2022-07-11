use twilight_model::id::{marker::MessageMarker, Id};

pub trait MultMapKey: Copy {
    fn index<const N: usize>(self) -> usize;
}

macro_rules! impl_separator {
    ($($ty:ty),*) => {
        $(
            impl MultMapKey for $ty {
                #[inline]
                fn index<const N: usize>(self) -> usize {
                    self as usize % N
                }
            }
        )*
    };
}

impl_separator!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl MultMapKey for Id<MessageMarker> {
    fn index<const N: usize>(self) -> usize {
        self.get() as usize % N
    }
}
