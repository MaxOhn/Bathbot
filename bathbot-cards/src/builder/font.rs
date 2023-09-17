use skia_safe::{
    font_style::{Slant, Weight, Width},
    Data, Font, FontStyle, Typeface,
};

use crate::error::FontError;

pub(crate) struct MissingStyle;
pub(crate) struct MissingFontData;
pub(crate) struct MissingSize;

pub(crate) struct FontData {
    normal: Vec<u8>,
    italic: Vec<u8>,
}

impl FontData {
    pub(crate) fn new(normal: Vec<u8>, italic: Vec<u8>) -> Self {
        Self { normal, italic }
    }
}

pub(crate) struct FontBuilder<ST, F, SI> {
    style: ST,
    data: F,
    size: SI,
}

impl FontBuilder<MissingStyle, MissingFontData, MissingSize> {
    pub(crate) fn new() -> Self {
        Self {
            style: MissingStyle,
            data: MissingFontData,
            size: MissingSize,
        }
    }
}

impl FontBuilder<FontStyle, &FontData, f32> {
    pub(crate) fn build(self) -> Result<Font, FontError> {
        let font_data = match self.style.slant() {
            Slant::Upright => self.data.normal.as_slice(),
            Slant::Italic => self.data.italic.as_slice(),
            Slant::Oblique => unimplemented!(),
        };

        // SAFETY: `self.font_data` outlives `Data`
        let data = unsafe { Data::new_bytes(font_data) };
        let typeface = Typeface::from_data(data, None).ok_or(FontError::Typeface)?;

        Ok(Font::new(typeface, Some(self.size)))
    }
}

impl<ST, F, SI> FontBuilder<ST, F, SI> {
    pub(crate) fn style(
        self,
        weight: impl Into<Weight>,
        width: Width,
        slant: Slant,
    ) -> FontBuilder<FontStyle, F, SI> {
        FontBuilder {
            style: FontStyle::new(weight.into(), width, slant),
            data: self.data,
            size: self.size,
        }
    }

    pub(crate) fn data(self, data: &FontData) -> FontBuilder<ST, &FontData, SI> {
        FontBuilder {
            style: self.style,
            data,
            size: self.size,
        }
    }

    pub(crate) fn size(self, size: f32) -> FontBuilder<ST, F, f32> {
        FontBuilder {
            style: self.style,
            data: self.data,
            size,
        }
    }
}
