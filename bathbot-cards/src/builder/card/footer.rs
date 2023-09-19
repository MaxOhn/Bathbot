use std::{fs, path::PathBuf};

use skia_safe::{font_style::Slant, utils::text_utils::Align, Data, Image, RRect, Rect, Vector};

use super::{CardBuilder, H, W};
use crate::{
    builder::{
        font::FontBuilder,
        paint::{Gradient, PaintBuilder},
    },
    card::CardInner,
    error::FooterError,
    font::FontData,
    svg::Svg,
};

pub(crate) const FOOTER_H: i32 = 184;
pub(crate) const FOOTER_LOGO_W: i32 = 587;
pub(crate) const FOOTER_TEXT_H: i32 = 35;
pub(crate) const FOOTER_TEXT_MARGIN: i32 = 35;
pub(crate) const FOOTER_DATE_MARGIN_RIGHT: i32 = 48;

impl CardBuilder<'_> {
    pub(crate) fn draw_footer(
        &mut self,
        card: &CardInner<'_>,
        font_data: &FontData,
    ) -> Result<&mut Self, FooterError> {
        draw_background(self)?;
        draw_logo(self, card.assets.clone())?;
        draw_name(self, card.assets.clone())?;
        draw_date(self, card.date, font_data)?;

        Ok(self)
    }
}

fn draw_background(card: &mut CardBuilder<'_>) -> Result<(), FooterError> {
    let rect = Rect::new(0.0, 0.0, W as f32, FOOTER_H as f32);
    let radii = [
        Vector::from((0.0, 0.0)),
        Vector::from((30.0, 30.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
    ];
    let rrect = RRect::new_rect_radii(rect, &radii);

    let start = Gradient {
        pos: ((W / 2) as f32, 0.0),
        argb: (153, 0, 0, 0),
    };
    let end = Gradient {
        pos: ((W / 2) as f32, FOOTER_H as f32),
        argb: (76, 0, 0, 0),
    };

    let paint = PaintBuilder::gradient(start, end)?.build();

    let translate_y = H - FOOTER_H;

    card.canvas
        .translate((0, translate_y))
        .draw_rrect(rrect, &paint)
        .translate((-0, -translate_y));

    Ok(())
}

fn draw_logo(card: &mut CardBuilder<'_>, mut assets: PathBuf) -> Result<(), FooterError> {
    assets.push("branding/icon.png");
    let bytes = fs::read(assets).map_err(FooterError::LogoFile)?;

    // SAFETY: `bytes` and `data` share the same lifetime
    let data = unsafe { Data::new_bytes(&bytes) };

    let img = Image::from_encoded_with_alpha_type(data, None).ok_or(FooterError::Icon)?;
    let scale: f32 = FOOTER_H as f32 / FOOTER_LOGO_W as f32;
    let y_pos = (scale.recip() * (H - FOOTER_H) as f32) as i32;

    card.canvas
        .scale((scale, scale))
        .draw_image(&img, (0, y_pos), None)
        .scale((scale.recip(), scale.recip()));

    Ok(())
}

fn draw_name(card: &mut CardBuilder<'_>, mut assets: PathBuf) -> Result<(), FooterError> {
    assets.push("branding/text.svg");
    let bytes = fs::read(assets).map_err(FooterError::BrandingFile)?;
    let svg = Svg::parse(&bytes).map_err(FooterError::BrandingSvg)?;
    let paint = PaintBuilder::rgb(255, 255, 255).anti_alias().build();
    let translate_x = FOOTER_H + FOOTER_TEXT_MARGIN;
    let translate_y = H - FOOTER_H + (FOOTER_H - FOOTER_TEXT_H) / 2 + 1;
    let scale_x = FOOTER_TEXT_H as f32 / svg.view_box_h as f32;
    let scale_y = FOOTER_TEXT_H as f32 / svg.view_box_h as f32;

    card.canvas
        .translate((translate_x, translate_y))
        .scale((scale_x, scale_y))
        .draw_path(&svg.path, &paint)
        .scale((scale_x.recip(), scale_y.recip()))
        .translate((-translate_x, -translate_y));

    Ok(())
}

fn draw_date(
    card: &mut CardBuilder<'_>,
    date: &str,
    font_data: &FontData,
) -> Result<(), FooterError> {
    let font = FontBuilder::build(200, Slant::Italic, font_data, 45.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let pos_x = W - FOOTER_DATE_MARGIN_RIGHT;
    let pos_y = H - FOOTER_H + 63 + 45;

    card.canvas.draw_str_align(
        date,
        (pos_x as f32, pos_y as f32),
        &font,
        &paint,
        Align::Right,
    );

    Ok(())
}
