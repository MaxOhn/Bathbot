use std::mem;

use skia_safe::{BlurStyle, Color, MaskFilter, Paint, Shader, TileMode};

use crate::error::PaintError;

pub(crate) struct Gradient {
    pub(crate) pos: (f32, f32),
    pub(crate) argb: (u8, u8, u8, u8),
}

pub(crate) struct PaintBuilder {
    paint: Paint,
}

impl PaintBuilder {
    pub(crate) fn build(&mut self) -> Paint {
        mem::take(&mut self.paint)
    }

    pub(crate) fn rgb(r: u8, g: u8, b: u8) -> Self {
        let mut paint = Paint::default();
        paint.set_argb(255, r, g, b);

        Self { paint }
    }

    pub(crate) fn alpha(&mut self, alpha: u8) -> &mut Self {
        self.paint.set_alpha(alpha);

        self
    }

    pub(crate) fn anti_alias(&mut self) -> &mut Self {
        self.paint.set_anti_alias(true);

        self
    }

    pub(crate) fn mask_filter(
        &mut self,
        style: BlurStyle,
        sigma: f32,
    ) -> Result<&mut Self, PaintError> {
        let mask_filter = MaskFilter::blur(style, sigma, None).ok_or(PaintError::MaskFilter)?;
        self.paint.set_mask_filter(Some(mask_filter));

        Ok(self)
    }

    pub(crate) fn gradient(start: Gradient, end: Gradient) -> Result<Self, PaintError> {
        let Gradient {
            pos: start_pos,
            argb: (a, r, g, b),
        } = start;
        let start_color = Color::from_argb(a, r, g, b);

        let Gradient {
            pos: end_pos,
            argb: (a, r, g, b),
        } = end;
        let end_color = Color::from_argb(a, r, g, b);

        let pos = (start_pos, end_pos);
        let colors: &[Color] = &[start_color, end_color];

        let shader = Shader::linear_gradient(pos, colors, None, TileMode::Mirror, None, None)
            .ok_or(PaintError::Gradient)?;

        let mut paint = Paint::default();
        paint.set_shader(Some(shader));

        Ok(Self { paint })
    }
}
