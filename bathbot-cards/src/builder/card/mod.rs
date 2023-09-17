mod footer;
mod header;
mod info;

use std::fs;

use itoa::Buffer;
use skia_safe::{Canvas, Data, Image};

use crate::{error::BackgroundError, skills::CardTitle, ASSETS_PATH};

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
    ) -> Result<&mut Self, BackgroundError> {
        let filename = title.prefix.filename();
        let path = format!("{ASSETS_PATH}backgrounds/{filename}.png");
        let bytes = fs::read(path).map_err(BackgroundError::File)?;

        // SAFETY: `bytes` and `Data` share the same lifetime
        let data = unsafe { Data::new_bytes(&bytes) };

        let img = Image::from_encoded_with_alpha_type(data, None).ok_or(BackgroundError::Image)?;
        self.canvas.draw_image(&img, (0, 0), None);

        Ok(self)
    }
}
