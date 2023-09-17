use std::fs;

use rosu_v2::model::GameMode;
use skia_safe::{
    font_style::{Slant, Width},
    scalar, Data, Image, RRect, Rect, Vector,
};

use crate::{
    builder::{
        card::CardBuilder,
        font::{FontBuilder, FontData},
        paint::{Gradient, PaintBuilder},
    },
    card::CardInner,
    error::HeaderError,
    skills::CardTitle,
    svg::Svg,
    ASSETS_PATH,
};

use super::W;

pub(crate) const HEADER_H: i32 = 250;
pub(crate) const HEADER_PAD_LEFT: i32 = 53;
pub(crate) const HEADER_FLAG_MARGIN_LEFT: i32 = 20;
pub(crate) const HEADER_MODE_W: i32 = 250;
pub(crate) const HEADER_MODE_H: i32 = 250;
pub(crate) const HEADER_FLAG_H: i32 = 70;

// TODO
const HEADER_TITLE_PAD_TOP: i32 = 23;

impl CardBuilder<'_> {
    pub(crate) fn draw_header(
        &mut self,
        mode: GameMode,
        data: &CardInner<'_>,
        title: &CardTitle,
        font_data: &FontData,
    ) -> Result<&mut Self, HeaderError> {
        draw_background(self)?;
        let height = draw_title(self, title, font_data)?;
        let len = draw_name(self, data.username, font_data, height)?;
        draw_flag(self, data.flag, height, len)?;
        draw_mode_background(self)?;
        draw_mode_icon(self, mode)?;

        Ok(self)
    }
}

fn draw_background(card: &mut CardBuilder<'_>) -> Result<(), HeaderError> {
    let rect = Rect::new(0.0, 0.0, W as f32, HEADER_H as f32);
    let radii = [
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((30.0, 30.0)),
    ];
    let rrect = RRect::new_rect_radii(rect, &radii);

    let start = Gradient {
        pos: ((W / 2) as f32, 0.0),
        argb: (171, 0, 0, 0),
    };
    let end = Gradient {
        pos: ((W / 2) as f32, HEADER_H as f32),
        argb: (204, 0, 0, 0),
    };

    let paint = PaintBuilder::gradient(start, end)?.build();
    card.canvas.draw_rrect(rrect, &paint);

    Ok(())
}

fn draw_title(
    card: &mut CardBuilder<'_>,
    title: &CardTitle,
    font_data: &FontData,
) -> Result<TitleHeight, HeaderError> {
    let font = FontBuilder::new()
        .style(600, Width::NORMAL, Slant::Italic)
        .data(font_data)
        .size(50.0)
        .build()?;

    let title_text = title.to_string();

    let rect = Rect::new(
        HEADER_PAD_LEFT as f32,
        HEADER_TITLE_PAD_TOP as f32,
        (W - HEADER_MODE_W + HEADER_PAD_LEFT) as f32,
        0.0,
    );

    let paint = PaintBuilder::rgb(255, 255, 255).alpha(204).build();
    let (space_w, _) = font.measure_str(" ", Some(&paint));
    let mut word_x = rect.left;
    let mut word_y = rect.top + font.size();

    for word in title_text.split_ascii_whitespace() {
        let (word_w, _) = font.measure_str(word, Some(&paint));

        if word_w <= rect.right - word_x {
            card.canvas.draw_str(word, (word_x, word_y), &font, &paint);
        } else {
            word_y += font.spacing();
            word_x = rect.left;
            card.canvas.draw_str(word, (word_x, word_y), &font, &paint);
        }

        word_x += word_w + space_w;
    }

    Ok(TitleHeight(word_y - rect.top))
}

fn draw_name(
    card: &mut CardBuilder<'_>,
    name: &str,
    font_data: &FontData,
    TitleHeight(title_height): TitleHeight,
) -> Result<NameLength, HeaderError> {
    let font = FontBuilder::new()
        .style(800, Width::NORMAL, Slant::Upright)
        .data(font_data)
        .size(70.0)
        .build()?;

    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let x_pos = HEADER_PAD_LEFT as f32;
    let y_pos = HEADER_TITLE_PAD_TOP as f32 + title_height;
    card.canvas.draw_str(name, (x_pos, y_pos), &font, &paint);
    let (name_len, _) = font.measure_str(name, Some(&paint));

    Ok(NameLength(name_len))
}

fn draw_flag(
    card: &mut CardBuilder<'_>,
    flag: &[u8],
    TitleHeight(title_height): TitleHeight,
    NameLength(name_len): NameLength,
) -> Result<(), HeaderError> {
    // SAFETY: `flag` has a longer lifetime than `Data`
    let flag_data = unsafe { Data::new_bytes(flag) };
    let flag_img = Image::from_encoded_with_alpha_type(flag_data, None).ok_or(HeaderError::Flag)?;
    let x_pos = (HEADER_PAD_LEFT + HEADER_FLAG_MARGIN_LEFT) as f32 + name_len;
    let y_pos = (HEADER_TITLE_PAD_TOP - HEADER_FLAG_H + 12) as f32 + title_height;
    card.canvas.draw_image(&flag_img, (x_pos, y_pos), None);

    Ok(())
}

fn draw_mode_background(card: &mut CardBuilder<'_>) -> Result<(), HeaderError> {
    let rect = Rect::new(0.0, 0.0, 200.0, 250.0);

    let radii = [
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((30.0, 30.0)),
    ];

    let rrect = RRect::new_rect_radii(rect, &radii);

    let start = Gradient {
        pos: (100.0, 0.0),
        argb: (26, 255, 255, 255),
    };
    let end = Gradient {
        pos: (100.0, 250.0),
        argb: (26, 0, 0, 0),
    };
    let paint = PaintBuilder::gradient(start, end)?.build();

    let translate_x = W - HEADER_MODE_W;
    card.canvas
        .translate((translate_x, 0))
        .draw_rrect(rrect, &paint)
        .translate((-translate_x, -0));

    let paint = PaintBuilder::rgb(255, 102, 170).alpha(64).build();
    let translate_x = W - HEADER_MODE_W;
    card.canvas
        .translate((translate_x, 0))
        .draw_rrect(rrect, &paint)
        .translate((-translate_x, -0));

    Ok(())
}

fn draw_mode_icon(card: &mut CardBuilder<'_>, mode: GameMode) -> Result<(), HeaderError> {
    let filename = match mode {
        GameMode::Osu => "Standard",
        GameMode::Taiko => "Taiko",
        GameMode::Catch => "Catch",
        GameMode::Mania => "Mania",
    };

    let path = format!("{ASSETS_PATH}gamemodes/{filename}.svg");
    let bytes = fs::read(path).map_err(HeaderError::ModeFile)?;
    let svg = Svg::parse(&bytes)?;

    let mode_paint = PaintBuilder::rgb(255, 255, 255)
        .alpha(204)
        .anti_alias()
        .build();

    let translate_x = W - svg.view_box_w - 27;
    let translate_y = (HEADER_MODE_H - svg.view_box_h) / 2;

    card.canvas
        .translate((translate_x, translate_y))
        .draw_path(&svg.path, &mode_paint)
        .translate((-translate_x, -translate_y));

    Ok(())
}

#[derive(Copy, Clone)]
struct TitleHeight(scalar);

#[derive(Copy, Clone)]
struct NameLength(scalar);
