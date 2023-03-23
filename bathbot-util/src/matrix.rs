use std::ops::{Index, IndexMut};

pub struct Matrix<T: Default + Copy> {
    inner: Box<[T]>,
    width: usize,
}

impl<T: Default + Copy> Matrix<T> {
    pub fn new(columns: usize, rows: usize) -> Matrix<T> {
        Matrix {
            inner: vec![T::default(); columns * rows].into_boxed_slice(),
            width: columns,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.inner.len() / self.width
    }

    pub fn count_neighbors(&self, x: usize, y: usize, cell: T) -> u8
    where
        T: Eq,
    {
        let h = self.height();
        let mut neighbors = 0;

        for cx in x.saturating_sub(1)..self.width.min(x + 2) {
            for cy in y.saturating_sub(1)..h.min(y + 2) {
                neighbors += ((cx != x || cy != y) && self[(cx, cy)] == cell) as u8;
            }
        }

        neighbors
    }
}

impl<T: Default + Copy> Index<(usize, usize)> for Matrix<T> {
    type Output = T;

    #[inline]
    fn index(&self, coords: (usize, usize)) -> &T {
        &self.inner[coords.1 * self.width + coords.0]
    }
}

impl<T: Default + Copy> IndexMut<(usize, usize)> for Matrix<T> {
    #[inline]
    fn index_mut(&mut self, coords: (usize, usize)) -> &mut T {
        &mut self.inner[coords.1 * self.width + coords.0]
    }
}
