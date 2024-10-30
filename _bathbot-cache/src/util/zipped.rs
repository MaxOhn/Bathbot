use std::mem;

#[derive(Default)]
/// Provides a way to collect an iterator of tuples into specified collections.
/// Similar to `Iterator::unzip` but more flexible.
pub(crate) struct Zipped<C1, C2> {
    left: C1,
    right: C2,
}

impl<C1, C2> Zipped<C1, C2> {
    pub fn into_parts(self) -> (C1, C2) {
        (self.left, self.right)
    }
}

impl<C1, T1, C2, T2> FromIterator<(T1, T2)> for Zipped<C1, C2>
where
    C1: Default + Extend<T1>,
    C2: Default + Extend<T2>,
{
    #[inline]
    fn from_iter<T: IntoIterator<Item = (T1, T2)>>(iter: T) -> Self {
        let mut tuple = (C1::default(), C2::default());
        tuple.extend(iter);
        let (left, right) = tuple;

        Self { left, right }
    }
}

impl<C1, T1, C2, T2> Extend<(T1, T2)> for Zipped<C1, C2>
where
    C1: Default + Extend<T1>,
    C2: Default + Extend<T2>,
{
    fn extend<T: IntoIterator<Item = (T1, T2)>>(&mut self, iter: T) {
        let Self { left, right } = self;

        let mut tuple = (mem::take(left), mem::take(right));
        tuple.extend(iter);
        let (left, right) = tuple;

        self.left = left;
        self.right = right;
    }
}
