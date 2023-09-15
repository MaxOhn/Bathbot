mod footer;
mod header;
mod info;

use std::{fs, str::from_utf8 as str_from_utf8};

use skia_safe::{Canvas, Data, Image, Path};

use crate::{
    card::User,
    error::{BackgroundError, FooterError, InfoError},
    skills::{CardTitle, Skills},
    ASSETS_PATH,
};

pub(crate) struct CardBuilder<'c> {
    canvas: &'c Canvas,
}

impl CardBuilder<'_> {
    pub(crate) const H: i32 = 1260;
    pub(crate) const W: i32 = 980;

    pub(crate) fn new(canvas: &Canvas) -> Self {
        Self { canvas }
    }

    pub(crate) fn background(&mut self, title: &CardTitle) -> Result<(), BackgroundError> {
        let filename = title.prefix.filename();
        let path = format!("{ASSETS_PATH}backgrounds/{filename}.png");
        let bytes = fs::read(&path)?;

        // SAFETY: `bytes` and `Data` share the same lifetime
        let data = unsafe { Data::new_bytes(&bytes) };

        let img = Image::from_encoded_with_alpha_type(data, None).ok_or(BackgroundError::Image)?;
        self.canvas.draw_image(&img, (0, 0), None);

        Ok(())
    }

    pub(crate) fn info(&mut self, user: &User<'_>, skills: &Skills) -> Result<(), InfoError> {
        Ok(())
    }

    pub(crate) fn footer(&mut self) -> Result<(), FooterError> {
        Ok(())
    }
}

fn parse_svg_path(svg: &[u8]) -> Option<Path> {}
