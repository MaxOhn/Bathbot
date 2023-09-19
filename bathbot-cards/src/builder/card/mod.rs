mod footer;
mod header;
mod info;

use std::{fs, path::PathBuf};

use itoa::Buffer;
use skia_safe::{Canvas, Data, Image};

use crate::{error::BackgroundError, skills::CardTitle};

pub(crate) const H: i32 = 1260;
pub(crate) const W: i32 = 980;

pub(crate) struct CardBuilder<'c> {
    canvas: &'c mut Canvas,
    int_buf: Buffer,
}

impl<'a> CardBuilder<'a> {
    pub(crate) fn new(canvas: &'a mut Canvas) -> Self {
        Self {
            canvas,
            int_buf: Buffer::new(),
        }
    }

    pub(crate) fn draw_background(
        &mut self,
        title: &CardTitle,
        mut assets: PathBuf,
    ) -> Result<&mut Self, BackgroundError> {
        assets.push("backgrounds");
        assets.push(title.prefix.filename());
        let bytes = fs::read(assets).map_err(BackgroundError::File)?;

        // SAFETY: `bytes` and `Data` share the same lifetime
        let data = unsafe { Data::new_bytes(&bytes) };

        let img = Image::from_encoded_with_alpha_type(data, None).ok_or(BackgroundError::Image)?;
        self.canvas.draw_image(&img, (0, 0), None);

        Ok(self)
    }
}
