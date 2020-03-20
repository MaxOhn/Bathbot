use std::ops::{Index, IndexMut};

pub struct Matrix<T>
where
    T: Default + Clone + Copy,
{
    vec: Vec<T>,
    width: usize,
}

impl<T> Matrix<T>
where
    T: Default + Clone + Copy + Eq,
{
    pub fn new(columns: usize, rows: usize) -> Matrix<T> {
        Matrix {
            vec: vec![T::default(); columns * rows],
            width: columns,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.vec.len() / self.width
    }

    pub fn count_neighbors(&self, x: usize, y: usize, n: T) -> u8 {
        let w = self.width;
        let h = self.height();
        let mut neighbors = 0;
        for cx in x.saturating_sub(1)..=x + 1 {
            for cy in y.saturating_sub(1)..=y + 1 {
                if (cx != x || cy != y) && cx < w && cy < h && self[(cx, cy)] == n {
                    neighbors += 1;
                }
            }
        }
        neighbors
    }
}

impl<T> Index<(usize, usize)> for Matrix<T>
where
    T: Default + Copy,
{
    type Output = T;

    fn index(&self, matrix_entry: (usize, usize)) -> &T {
        &self.vec[matrix_entry.1 * self.width + matrix_entry.0]
    }
}

impl<T> IndexMut<(usize, usize)> for Matrix<T>
where
    T: Default + Copy,
{
    fn index_mut(&mut self, matrix_entry: (usize, usize)) -> &mut T {
        &mut self.vec[matrix_entry.1 * self.width + matrix_entry.0]
    }
}
