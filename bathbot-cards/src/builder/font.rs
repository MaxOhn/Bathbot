use skia_safe::{font_style::Slant, Font, FontMgr};

use crate::{error::FontError, font::FontData};

pub(crate) struct FontBuilder;

impl FontBuilder {
    pub(crate) fn build(
        weight: i32,
        slant: Slant,
        data: &FontData,
        size: f32,
    ) -> Result<Font, FontError> {
        let font_data = data.get(weight.into(), slant);

        let typeface = FontMgr::new()
            .new_from_data(font_data, None)
            .ok_or(FontError::Typeface)?;

        Ok(Font::new(typeface, Some(size)))
    }
}
