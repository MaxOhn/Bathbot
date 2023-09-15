use skia_safe::{
    font_style::{Slant, Weight, Width},
    Font, FontStyle, Typeface,
};

pub(crate) struct MissingStyle;
pub(crate) struct MissingFontFamily;
pub(crate) struct MissingSize;

pub(crate) struct FontBuilder<ST, F, SI> {
    style: ST,
    font_family: F,
    size: SI,
}

impl FontBuilder<MissingStyle, MissingFontFamily, MissingSize> {
    pub(crate) fn new() -> Self {
        Self {
            style: MissingStyle,
            font_family: MissingFontFamily,
            size: MissingSize,
        }
    }
}

impl FontBuilder<FontStyle, &str, f32> {
    pub(crate) fn build(self) -> Option<Font> {
        let typeface = Typeface::new(self.font_family, self.style)?;

        Some(Font::new(typeface, Some(self.size)))
    }
}

impl<ST, F, SI> FontBuilder<ST, F, SI> {
    pub(crate) fn style(
        mut self,
        weight: impl Into<Weight>,
        width: Width,
        slant: Slant,
    ) -> FontBuilder<FontStyle, F, SI> {
        FontBuilder {
            style: FontStyle::new(weight, width, slant),
            font_family: self.font_family,
            size: self.size,
        }
    }

    pub(crate) fn family(mut self, family: &str) -> FontBuilder<ST, &str, SI> {
        FontBuilder {
            style: self.style,
            font_family: family,
            size: self.size,
        }
    }

    pub(crate) fn size(mut self, size: f32) -> FontBuilder<ST, F, f32> {
        FontBuilder {
            style: self.style,
            font_family: self.font_family,
            size,
        }
    }
}
