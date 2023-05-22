use std::io::Cursor;

use eyre::{Result, WrapErr};
use image::{DynamicImage, GenericImageView, ImageOutputFormat::Png};
use rand::RngCore;

pub struct ImageReveal {
    dim: (u32, u32),
    original: DynamicImage,
    x: u32,
    y: u32,
    radius: u32,
}

impl ImageReveal {
    pub fn new(original: DynamicImage) -> Self {
        let (w, h) = original.dimensions();
        let radius = 100;
        let mut rng = rand::thread_rng();
        let x = radius + rng.next_u32() % (w - 2 * radius);
        let y = radius + rng.next_u32() % (h - 2 * radius);

        Self {
            dim: (w, h),
            original,
            x,
            y,
            radius,
        }
    }

    pub fn increase_radius(&mut self) {
        self.radius += 75;
    }

    pub fn sub_image(&self) -> Result<Vec<u8>> {
        let cx = self.x.saturating_sub(self.radius);
        let cy = self.y.saturating_sub(self.radius);
        let (w, h) = self.dim;
        let w = (self.x + self.radius).min(w) - cx;
        let h = (self.y + self.radius).min(h) - cy;
        let sub_image = self.original.crop_imm(cx, cy, w, h);
        let png_bytes: Vec<u8> = Vec::with_capacity((w * h) as usize);

        let mut cursor = Cursor::new(png_bytes);
        sub_image
            .write_to(&mut cursor, Png)
            .wrap_err("Failed to encode image")?;

        Ok(cursor.into_inner())
    }
}
