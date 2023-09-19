use std::{fs, path::PathBuf};

use rosu_v2::model::GameMode;
use skia_safe::{font_style::Slant, scalar, Data, Image, RRect, Rect, TextBlobBuilder, Vector};

use super::W;
use crate::{
    builder::{
        card::CardBuilder,
        font::FontBuilder,
        paint::{Gradient, PaintBuilder},
    },
    card::CardInner,
    error::HeaderError,
    font::FontData,
    skills::CardTitle,
    svg::Svg,
};

pub(crate) const HEADER_H: i32 = 250;
pub(crate) const HEADER_PAD_LEFT: i32 = 53;
pub(crate) const HEADER_NAME_MARGIN_TOP: i32 = -5;
pub(crate) const HEADER_FLAG_MARGIN_LEFT: i32 = 20;
pub(crate) const HEADER_MODE_W: i32 = 200;
pub(crate) const HEADER_MODE_H: i32 = 250;
pub(crate) const HEADER_FLAG_H: i32 = 70;

impl CardBuilder<'_> {
    pub(crate) fn draw_header(
        &mut self,
        mode: GameMode,
        data: &CardInner<'_>,
        title: &CardTitle,
        font_data: &FontData,
    ) -> Result<&mut Self, HeaderError> {
        draw_background(self)?;
        let title = draw_title(self, title, data.username, font_data)?;
        draw_flag(self, data.flag, title)?;
        draw_mode_background(self, mode)?;
        draw_mode_icon(self, mode, data.assets.clone())?;

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
    name: &str,
    font_data: &FontData,
) -> Result<Title, HeaderError> {
    let title_text = title.to_string();

    let title_w = (W - HEADER_MODE_W - 2 * HEADER_PAD_LEFT) as f32;

    let font = FontBuilder::build(600, Slant::Italic, font_data, 50.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(204).build();

    let space_iter = title_text
        .bytes()
        .enumerate()
        .filter_map(|(i, b)| (b == b' ').then_some(i));

    let mut row_start = 0;
    let mut last_space_idx = 0;
    let mut row_y = font.size();

    let mut builder = TextBlobBuilder::new();

    // Write as many words as possible as long as the line fits within the title
    // width
    for space_idx in space_iter {
        let candidate = &title_text[row_start..space_idx];
        let (candidate_w, _) = font.measure_str(candidate, Some(&paint));

        if candidate_w > title_w {
            let glyphs = builder.alloc_run(&font, last_space_idx - row_start, (0.0, row_y), None);
            let row = &title_text[row_start..last_space_idx];
            font.str_to_glyphs(row, glyphs);
            row_start = last_space_idx + 1;
            row_y += font.spacing();
        }

        last_space_idx = space_idx;
    }

    // Write the remaining words
    let candidate = &title_text[row_start..];
    let (candidate_w, _) = font.measure_str(candidate, Some(&paint));

    if candidate_w > title_w {
        // The very last word does not fit with the rest of the last line
        let row = &title_text[row_start..last_space_idx];
        let glyphs = builder.alloc_run(&font, last_space_idx - row_start, (0.0, row_y), None);
        font.str_to_glyphs(row, glyphs);

        row_y += font.spacing();

        let row = &title_text[last_space_idx + 1..];
        let glyphs = builder.alloc_run(
            &font,
            title_text.len() - last_space_idx - 1,
            (0.0, row_y),
            None,
        );
        font.str_to_glyphs(row, glyphs);
    } else {
        // The line after `row_start` fits entirely within the rect
        let glyphs = builder.alloc_run(&font, title_text.len() - row_start, (0.0, row_y), None);
        font.str_to_glyphs(candidate, glyphs);
    }

    let blob = builder.make().ok_or(HeaderError::TitleTextBlob)?;

    let name_font = FontBuilder::build(800, Slant::Upright, font_data, 70.0)?;
    let name_paint = PaintBuilder::rgb(255, 255, 255).build();

    let title_h = row_y + name_font.size() + (name_font.spacing() - name_font.size())
        - HEADER_NAME_MARGIN_TOP as f32;

    let title_y = (HEADER_H as f32 - title_h) / 2.0;

    card.canvas
        .draw_text_blob(blob, (HEADER_PAD_LEFT as f32, title_y), &paint);

    let pos_x = HEADER_PAD_LEFT as f32;
    let pos_y = title_y + row_y + name_font.size() - HEADER_NAME_MARGIN_TOP as f32;

    card.canvas
        .draw_str(name, (pos_x, pos_y), &name_font, &name_paint);

    let (name_len, _) = name_font.measure_str(name, Some(&name_paint));

    Ok(Title {
        name_len,
        title_height: title_y + row_y + name_font.size() - HEADER_NAME_MARGIN_TOP as f32,
    })
}

fn draw_flag(card: &mut CardBuilder<'_>, flag: &[u8], title: Title) -> Result<(), HeaderError> {
    // SAFETY: `flag` has a longer lifetime than `Data`
    let flag_data = unsafe { Data::new_bytes(flag) };
    let flag_img = Image::from_encoded_with_alpha_type(flag_data, None).ok_or(HeaderError::Flag)?;
    let pos_x = (HEADER_PAD_LEFT + HEADER_FLAG_MARGIN_LEFT) as f32 + title.name_len;
    let pos_y = (-HEADER_FLAG_H + 12) as f32 + title.title_height;
    card.canvas.draw_image(&flag_img, (pos_x, pos_y), None);

    Ok(())
}

fn draw_mode_background(card: &mut CardBuilder<'_>, mode: GameMode) -> Result<(), HeaderError> {
    let rect_w = HEADER_MODE_W as f32;
    let rect = Rect::new(0.0, 0.0, rect_w, HEADER_MODE_H as f32);

    let radii = [
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((0.0, 0.0)),
        Vector::from((30.0, 30.0)),
    ];

    let rrect = RRect::new_rect_radii(rect, &radii);

    let start = Gradient {
        pos: (rect_w / 2.0, 0.0),
        argb: (26, 255, 255, 255),
    };
    let end = Gradient {
        pos: (rect_w / 2.0, 250.0),
        argb: (26, 0, 0, 0),
    };
    let paint = PaintBuilder::gradient(start, end)?.build();

    let translate_x = W - HEADER_MODE_W;
    card.canvas
        .translate((translate_x, 0))
        .draw_rrect(rrect, &paint)
        .translate((-translate_x, -0));

    let (r, g, b) = match mode {
        GameMode::Osu => (255, 102, 170),
        GameMode::Taiko => (94, 203, 162),
        GameMode::Catch => (102, 204, 255),
        GameMode::Mania => (197, 102, 255),
    };

    let paint = PaintBuilder::rgb(r, g, b).alpha(64).build();
    let translate_x = W - HEADER_MODE_W;
    card.canvas
        .translate((translate_x, 0))
        .draw_rrect(rrect, &paint)
        .translate((-translate_x, -0));

    Ok(())
}

fn draw_mode_icon(
    card: &mut CardBuilder<'_>,
    mode: GameMode,
    mut assets: PathBuf,
) -> Result<(), HeaderError> {
    let filename = match mode {
        GameMode::Osu => "Standard.svg",
        GameMode::Taiko => "Taiko.svg",
        GameMode::Catch => "Catch.svg",
        GameMode::Mania => "Mania.svg",
    };

    assets.push("gamemodes");
    assets.push(filename);
    let bytes = fs::read(assets).map_err(HeaderError::ModeFile)?;
    let svg = Svg::parse(&bytes)?;

    let mode_paint = PaintBuilder::rgb(255, 255, 255)
        .alpha(204)
        .anti_alias()
        .build();

    let translate_x = W - HEADER_MODE_W + (HEADER_MODE_W - svg.view_box_w) / 2;
    let translate_y = (HEADER_MODE_H - svg.view_box_h) / 2;

    card.canvas
        .translate((translate_x, translate_y))
        .draw_path(&svg.path, &mode_paint)
        .translate((-translate_x, -translate_y));

    Ok(())
}

#[derive(Copy, Clone)]
struct Title {
    name_len: scalar,
    title_height: scalar,
}
