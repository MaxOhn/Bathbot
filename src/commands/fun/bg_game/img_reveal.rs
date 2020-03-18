use crate::Error;

use image::{png::PNGEncoder, ColorType, DynamicImage, GenericImageView};
use rand::RngCore;

pub struct ImageReveal {
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
            original,
            x,
            y,
            radius,
        }
    }

    pub fn increase_radius(&mut self) {
        self.radius += 75;
    }

    pub fn sub_image(&self) -> Result<Vec<u8>, Error> {
        let cx = self.x.saturating_sub(self.radius);
        let cy = self.y.saturating_sub(self.radius);
        let (w, h) = self.original.dimensions();
        let w = (self.x + self.radius).min(w) - cx;
        let h = (self.y + self.radius).min(h) - cy;
        let sub_image = self.original.view(cx, cy, w, h).to_image().into_vec();
        let mut png_bytes: Vec<u8> = Vec::with_capacity((w * h) as usize);
        let png_encoder = PNGEncoder::new(&mut png_bytes);
        png_encoder.encode(&sub_image, w, h, ColorType::Rgba8)?;
        Ok(png_bytes)
    }

    pub fn full(&self) -> Result<Vec<u8>, Error> {
        let (w, h) = self.original.dimensions();
        let mut png_bytes: Vec<u8> = Vec::with_capacity((w * h) as usize);
        let png_encoder = PNGEncoder::new(&mut png_bytes);
        // doesn't work without next line for some reason
        let img = self.original.view(0, 0, w, h).to_image().into_vec();
        png_encoder.encode(&img, w, h, ColorType::Rgba8)?;
        Ok(png_bytes)
    }
}

impl Default for ImageReveal {
    fn default() -> Self {
        let original = DynamicImage::new_rgba8(1, 1);
        Self {
            original,
            x: 0,
            y: 0,
            radius: 0,
        }
    }
}
