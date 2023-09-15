use std::fs;

use rosu_v2::model::GameMode;
use skia_safe::font_style::{Slant, Width};
use skia_safe::{scalar, Data, Image, RRect, Rect, Vector};

use crate::builder::card::parse_svg_path;
use crate::builder::font::FontBuilder;
use crate::error::{FontError, PaintError};
use crate::{
    builder::{
        card::CardBuilder,
        paint::{Gradient, PaintBuilder},
    },
    card::User,
    error::HeaderError,
    skills::CardTitle,
    ASSETS_PATH,
};

const HEADER_H: i32 = 250;
const HEADER_PAD_LEFT: i32 = 53;
const HEADER_FLAG_MARGIN_LEFT: i32 = 20;
const HEADER_MODE_W: i32 = 250;
const HEADER_MODE_H: i32 = 250;
const HEADER_FLAG_H: i32 = 70;

// TODO
const HEADER_TITLE_PAD_TOP: i32 = 23;

impl CardBuilder<'_> {
    pub(crate) fn header(
        &mut self,
        mode: GameMode,
        user: &User<'_>,
        title: &CardTitle,
    ) -> Result<(), HeaderError> {
        self.draw_background()?;
        let height = self.draw_title(title)?;
        let len = self.draw_name(user.name, height)?;
        self.draw_flag(user.flag, height, len)?;
        self.draw_mode_background()?;
        self.draw_mode_icon(mode)?;

        Ok(())
    }

    fn draw_background(&mut self) -> Result<(), HeaderError> {
        let rect = Rect::new(0.0, 0.0, Self::W as f32, HEADER_H as f32);
        let radii = [
            Vector::from((0.0, 0.0)),
            Vector::from((0.0, 0.0)),
            Vector::from((0.0, 0.0)),
            Vector::from((30.0, 30.0)),
        ];
        let rrect = RRect::new_rect_radii(rect, &radii);
        let start = Gradient {
            pos: ((Self::W / 2) as f32, 0.0),
            argb: (171, 0, 0, 0),
        };
        let end = Gradient {
            pos: ((Self::W / 2) as f32, HEADER_H as f32),
            argb: (204, 0, 0, 0),
        };

        let paint = PaintBuilder::gradient(start, end)
            .ok_or(PaintError::Gradient)?
            .build();

        self.canvas.draw_rrect(rrect, &paint);

        Ok(())
    }

    fn draw_title(&mut self, title: &CardTitle) -> Result<TitleHeight, HeaderError> {
        let font = FontBuilder::new()
            .style(600, Width::NORMAL, Slant::Italic)
            .family("Roboto")
            .size(50.0)
            .build()
            .ok_or(FontError::Typeface)?;

        let title_text = title.to_string();

        let rect = Rect::new(
            HEADER_PAD_LEFT as f32,
            HEADER_TITLE_PAD_TOP as f32,
            (Self::W - HEADER_MODE_W + HEADER_PAD_LEFT) as f32,
            0.0,
        );

        let paint = PaintBuilder::rgb(255, 255, 255).alpha(204).build();
        let (space_w, _) = font.measure_str(" ", Some(&paint));
        let mut word_x = rect.left;
        let mut word_y = rect.top + font.size();
        let mut height = font.size();

        for word in title_text.split_ascii_whitespace() {
            let (word_w, _) = font.measure_str(word, Some(&paint));

            if word_w <= rect.right - word_x {
                self.canvas.draw_str(word, (word_x, word_y), &font, &paint);
            } else {
                word_y += font.spacing();
                height += font.spacing();
                word_x = rect.left;
                self.canvas.draw_str(word, (word_x, word_y), &font, &paint);
            }

            word_x += word_w + space_w;
        }

        Ok(TitleHeight(height))
    }

    fn draw_name(
        &mut self,
        name: &str,
        TitleHeight(title_height): TitleHeight,
    ) -> Result<NameLength, HeaderError> {
        let font = FontBuilder::new()
            .style(800, Width::NORMAL, Slant::Upright)
            .family("Roboto")
            .size(70.0)
            .build()
            .ok_or(FontError::Typeface)?;

        let paint = PaintBuilder::rgb(255, 255, 255).build();
        let x_pos = HEADER_PAD_LEFT as f32;
        let y_pos = HEADER_TITLE_PAD_TOP as f32 + title_height;
        self.canvas.draw_str(name, (x_pos, y_pos), &font, &paint);
        let (name_len, _) = font.measure_str(name, Some(&paint));

        Ok(NameLength(name_len))
    }

    fn draw_flag(
        &mut self,
        flag: &[u8],
        TitleHeight(title_height): TitleHeight,
        NameLength(name_len): NameLength,
    ) -> Result<(), HeaderError> {
        // SAFETY: `flag` has a longer lifetime than `Data`
        let flag_data = unsafe { Data::new_bytes(flag) };
        let flag_img =
            Image::from_encoded_with_alpha_type(flag_data, None).ok_or(HeaderError::Flag)?;
        let x_pos = (HEADER_PAD_LEFT + HEADER_FLAG_MARGIN_LEFT) as f32 + name_len;
        let y_pos = (HEADER_TITLE_PAD_TOP - HEADER_FLAG_H + 12) as f32 + title_height;
        self.canvas.draw_image(&flag_img, (x_pos, y_pos), None);

        Ok(())
    }

    fn draw_mode_background(&mut self) -> Result<(), HeaderError> {
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
        let paint = PaintBuilder::gradient(start, end).build();

        let translate_x = Self::W - HEADER_MODE_W;
        self.canvas
            .translate((translate_x, 0))
            .draw_rrect(rrect, &paint)
            .translate((-translate_x, -0));

        let paint = PaintBuilder::rgb(255, 102, 170).alpha(64).build();
        let translate_x = Self::W - HEADER_MODE_W;
        self.canvas
            .translate((translate_x, 0))
            .draw_rrect(rrect, &paint)
            .translate((-translate_x, -0));

        Ok(())
    }

    fn draw_mode_icon(&mut self, mode: GameMode) -> Result<(), HeaderError> {
        let filename = match mode {
            GameMode::Osu => "Standard",
            GameMode::Taiko => "Taiko",
            GameMode::Catch => "Catch",
            GameMode::Mania => "Mania",
        };

        let path = format!("{ASSETS_PATH}gamemodes/filename.svg");
        let bytes = fs::read(&path).map_err(HeaderError::ModeFile)?;

        let mode_path = parse_svg_path(&bytes).ok_or(HeaderError::ModePath)?;

        let mode_paint = PaintBuilder::rgb(255, 255, 255)
            .alpha(204)
            .anti_alias()
            .build();

        let translate_x = Self::W - 144 - 27;
        let translate_y = (HEADER_MODE_H - 144) / 2;

        self.canvas
            .translate((translate_x, translate_y))
            .draw_path(&mode_path, &mode_paint)
            .translate((-translate_x, -translate_y));

        Ok(())
    }
}

#[derive(Copy, Clone)]
struct TitleHeight(scalar);

#[derive(Copy, Clone)]
struct NameLength(scalar);
