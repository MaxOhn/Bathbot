use skia_safe::{font_style::Slant, Data, Font, Typeface};

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

        // SAFETY: `self.font_data` outlives `Data`
        let data = unsafe { Data::new_bytes(font_data) };
        let typeface = Typeface::from_data(data, None).ok_or(FontError::Typeface)?;

        Ok(Font::new(typeface, Some(size)))
    }
}
