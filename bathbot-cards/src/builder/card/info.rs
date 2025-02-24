use std::cmp;

use skia_safe::{
    BlurStyle, ClipOp, Data, ISize, Image, RRect, Rect, TextBlobBuilder, font_style::Slant,
    utils::text_utils::Align,
};

use super::{CardBuilder, H, W, footer::FOOTER_H, header::HEADER_H};
use crate::{
    builder::{
        font::FontBuilder,
        paint::{Gradient, PaintBuilder},
    },
    card::CardInner,
    error::InfoError,
    font::FontData,
    skills::Skills,
};

const INFO_PAD: i32 = 53;
const INFO_UPPER_LEFT_W: i32 = 445;
const INFO_UPPER_H: i32 = 571;
const INFO_UPPER_MARGIN: i32 = 13;
const INFO_UPPER_RIGHT_W: i32 = W - 2 * INFO_PAD - INFO_UPPER_MARGIN - INFO_UPPER_LEFT_W;
const INFO_LOWER_MARGIN: i32 = 13;
const INFO_LOWER_W: i32 = W - 2 * INFO_PAD;
const INFO_LOWER_H: i32 =
    H - (HEADER_H + 2 * INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + FOOTER_H);
const INFO_AVATAR_W: i32 = INFO_UPPER_LEFT_W;
const INFO_AVATAR_H: i32 = INFO_AVATAR_W;
const INFO_GLOBAL_RANK_PAD: i32 = 20;

impl CardBuilder<'_> {
    pub(crate) fn draw_info(
        &mut self,
        card: &CardInner<'_>,
        skills: &Skills,
        font_data: &FontData,
    ) -> Result<&mut Self, InfoError> {
        draw_upper_left_background(self)?;
        draw_upper_right_background(self)?;
        draw_bottom_background(self)?;
        draw_pfp(self, card.pfp)?;
        draw_global_rank(self, card.rank_global, font_data)?;
        draw_country_rank(self, card.rank_country, font_data)?;
        draw_skills(self, skills, font_data)?;
        draw_level(self, card.level, font_data)?;
        draw_medals(self, card.medals, card.total_medals, font_data)?;

        Ok(self)
    }
}

fn draw_upper_left_background(card: &mut CardBuilder<'_>) -> Result<(), InfoError> {
    let rect = Rect::new(0.0, 0.0, INFO_UPPER_LEFT_W as f32, INFO_UPPER_H as f32);
    let paint = PaintBuilder::rgb(0, 0, 0).alpha(51).build();
    let translate_x = INFO_PAD;
    let translate_y = HEADER_H + INFO_PAD;

    card.canvas
        .translate((translate_x, translate_y))
        .draw_round_rect(rect, 16.0, 16.0, &paint)
        .translate((-translate_x, -translate_y));

    Ok(())
}

fn draw_upper_right_background(card: &mut CardBuilder<'_>) -> Result<(), InfoError> {
    let rect = Rect::new(0.0, 0.0, INFO_UPPER_RIGHT_W as f32, INFO_UPPER_H as f32);

    let start = Gradient {
        pos: ((INFO_UPPER_RIGHT_W / 2) as f32, 0.0),
        argb: (102, 0, 0, 0),
    };
    let end = Gradient {
        pos: ((INFO_UPPER_RIGHT_W / 2) as f32, INFO_UPPER_H as f32),
        argb: (45, 0, 0, 0),
    };
    let paint = PaintBuilder::gradient(start, end)?.build();
    let translate_x = INFO_PAD + INFO_UPPER_LEFT_W + INFO_UPPER_MARGIN;
    let translate_y = HEADER_H + INFO_PAD;

    card.canvas
        .translate((translate_x, translate_y))
        .draw_round_rect(rect, 16.0, 16.0, &paint)
        .translate((-translate_x, -translate_y));

    Ok(())
}

fn draw_bottom_background(card: &mut CardBuilder<'_>) -> Result<(), InfoError> {
    let rect = Rect::new(0.0, 0.0, INFO_LOWER_W as f32, INFO_LOWER_H as f32);
    let start = Gradient {
        pos: ((W / 2) as f32, 0.0),
        argb: (45, 0, 0, 0),
    };
    let end = Gradient {
        pos: ((W / 2) as f32, INFO_LOWER_H as f32),
        argb: (102, 0, 0, 0),
    };
    let paint = PaintBuilder::gradient(start, end)?.build();
    let translate_x = INFO_PAD;
    let translate_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN;

    card.canvas
        .translate((translate_x, translate_y))
        .draw_round_rect(rect, 16.0, 16.0, &paint)
        .translate((-translate_x, -translate_y));

    Ok(())
}

fn draw_pfp(card: &mut CardBuilder<'_>, pfp: &[u8]) -> Result<(), InfoError> {
    // SAFETY: `pfp` has a longer lifetime than `Data`
    let data = unsafe { Data::new_bytes(pfp) };

    let img = Image::from_encoded_with_alpha_type(data, None).ok_or(InfoError::Avatar)?;

    let ISize {
        width: img_w,
        height: img_h,
    } = img.dimensions();

    let max = cmp::max(img_w, img_h);
    let scale = INFO_AVATAR_W as f32 / max as f32;

    let offset_x = INFO_AVATAR_W as f32 - scale * img_w as f32;
    let offset_y = INFO_AVATAR_H as f32 - scale * img_h as f32;

    let pos_x = scale.recip() * (INFO_PAD as f32 + offset_x / 2.0);
    let pos_y = scale.recip() * ((HEADER_H + INFO_PAD) as f32 + offset_y / 2.0);

    let rect = Rect::new(
        INFO_PAD as f32,
        (HEADER_H + INFO_PAD) as f32,
        (INFO_PAD + INFO_AVATAR_W) as f32,
        (HEADER_H + INFO_PAD + INFO_AVATAR_H) as f32,
    );
    let rrect = RRect::new_rect_xy(rect, 16.0, 16.0);

    card.canvas.save();

    card.canvas
        .clip_rrect(rrect, Some(ClipOp::Intersect), Some(true))
        .scale((scale, scale))
        .draw_image(&img, (pos_x, pos_y), None)
        .restore();

    Ok(())
}

fn draw_global_rank(
    card: &mut CardBuilder<'_>,
    rank: u32,
    font_data: &FontData,
) -> Result<(), InfoError> {
    let rank = format!("#{rank}");
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let font = FontBuilder::build(400, Slant::Italic, font_data, 32.0)?;

    let pos_x = INFO_PAD + INFO_GLOBAL_RANK_PAD;
    let pos_y = HEADER_H + INFO_PAD + INFO_AVATAR_W + INFO_GLOBAL_RANK_PAD + 31;
    card.canvas
        .draw_str("Global", (pos_x as f32, pos_y as f32), &font, &paint);

    let font = FontBuilder::build(900, Slant::Upright, font_data, 45.0)?;

    let pos_x = INFO_PAD + INFO_GLOBAL_RANK_PAD;
    let pos_y = HEADER_H + INFO_PAD + INFO_AVATAR_W + INFO_GLOBAL_RANK_PAD + 31 + 44;
    card.canvas
        .draw_str(&rank, (pos_x as f32, pos_y as f32), &font, &paint);

    Ok(())
}

fn draw_country_rank(
    card: &mut CardBuilder<'_>,
    rank: u32,
    font_data: &FontData,
) -> Result<(), InfoError> {
    let rank = format!("#{rank}");
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let font = FontBuilder::build(300, Slant::Italic, font_data, 27.0)?;

    let pos_x = INFO_PAD + INFO_UPPER_LEFT_W - INFO_GLOBAL_RANK_PAD;
    let pos_y = HEADER_H + INFO_PAD + INFO_AVATAR_W + INFO_GLOBAL_RANK_PAD + 33;

    card.canvas.draw_str_align(
        "Country",
        (pos_x as f32, pos_y as f32),
        &font,
        &paint,
        Align::Right,
    );

    let font = FontBuilder::build(600, Slant::Upright, font_data, 37.0)?;

    let pos_x = INFO_PAD + INFO_UPPER_LEFT_W - INFO_GLOBAL_RANK_PAD;
    let pos_y = HEADER_H + INFO_PAD + INFO_AVATAR_W + INFO_GLOBAL_RANK_PAD + 33 + 37;

    card.canvas.draw_str_align(
        &rank,
        (pos_x as f32, pos_y as f32),
        &font,
        &paint,
        Align::Right,
    );

    Ok(())
}

fn draw_skills(
    card: &mut CardBuilder<'_>,
    skills: &Skills,
    font_data: &FontData,
) -> Result<(), InfoError> {
    struct DrawableSkill {
        name: &'static str,
        value: f64,
    }

    impl DrawableSkill {
        fn new(name: &'static str, value: f64) -> Self {
            Self { name, value }
        }
    }

    let drawables = match skills {
        Skills::Osu { acc, aim, speed } => {
            let acc = DrawableSkill::new("ACCURACY", *acc);
            let aim = DrawableSkill::new("AIM", *aim);
            let speed = DrawableSkill::new("SPEED", *speed);

            vec![acc, aim, speed]
        }
        Skills::Taiko { acc, strain } => {
            let acc = DrawableSkill::new("ACCURACY", *acc);
            let strain = DrawableSkill::new("STRAIN", *strain);

            vec![acc, strain]
        }
        Skills::Catch { acc, movement } => {
            let acc = DrawableSkill::new("ACCURACY", *acc);
            let movement = DrawableSkill::new("MOVEMENT", *movement);

            vec![acc, movement]
        }
        Skills::Mania { acc, strain } => {
            let acc = DrawableSkill::new("ACCURACY", *acc);
            let strain = DrawableSkill::new("STRAIN", *strain);

            vec![acc, strain]
        }
    };

    // `init_y`: y-pos of skill's rect
    // `margin`: pixels inbetween two rects' y-pos
    let (init_y, margin) = match drawables.len() {
        2 => (HEADER_H + INFO_PAD + 89, 270),
        3 => (HEADER_H + INFO_PAD + 44, 180),
        _ => unreachable!(),
    };

    let paint = PaintBuilder::rgb(255, 255, 255).build();

    let rect_x = INFO_PAD + INFO_UPPER_LEFT_W + INFO_UPPER_MARGIN + 30;

    let name_font = FontBuilder::build(300, Slant::Italic, font_data, 42.0)?;
    let name_x = INFO_PAD + INFO_UPPER_LEFT_W + INFO_UPPER_MARGIN + 46;

    let trunc_font = FontBuilder::build(900, Slant::Upright, font_data, 67.0)?;
    let trunc_x = INFO_PAD + INFO_UPPER_LEFT_W + INFO_UPPER_MARGIN + 30;

    let fract_font = FontBuilder::build(400, Slant::Upright, font_data, 67.0)?;

    for (skill, i) in drawables.into_iter().zip(0..) {
        let DrawableSkill { name, value } = skill;

        // Rectangle
        let rect = Rect::new(0.0, 0.0, 4.0, 43.0);
        let rect_y = init_y + i * margin;

        card.canvas
            .translate((rect_x, rect_y))
            .draw_rect(rect, &paint)
            .translate((-rect_x, -rect_y));

        // Name
        let name_y = init_y + 39 + i * margin;
        card.canvas
            .draw_str(name, (name_x as f32, name_y as f32), &name_font, &paint);

        // Value
        let mut builder = TextBlobBuilder::new();
        let trunc = format!("{}.", value.trunc() as i32);
        let fract = format!("{:0>2}", (value.fract() * 100.0) as i32);

        let trunc_y = init_y + 106 + i * margin;
        let trunc_glyphs = builder.alloc_run(&trunc_font, trunc.len(), (0.0, 0.0), None);
        trunc_font.str_to_glyphs(&trunc, trunc_glyphs);
        let (trunc_w, _) = trunc_font.measure_str(&trunc, Some(&paint));

        let fract_glyphs = builder.alloc_run(&fract_font, fract.len(), (trunc_w, 0.0), None);
        fract_font.str_to_glyphs(&fract, fract_glyphs);
        let blob = builder.make().ok_or(InfoError::SkillTextBlob)?;
        card.canvas
            .draw_text_blob(blob, (trunc_x as f32, trunc_y as f32), &paint);
    }

    Ok(())
}

fn draw_level(
    card: &mut CardBuilder<'_>,
    level: f32,
    font_data: &FontData,
) -> Result<(), InfoError> {
    // Text
    let level_value = card.int_buf.format(level.trunc() as u32);
    let percent = level.fract();

    let font = FontBuilder::build(300, Slant::Italic, font_data, 35.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(168).build();

    let pos_x = INFO_PAD + 32;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 46;
    let level_text = "Level";
    card.canvas
        .draw_str(level_text, (pos_x as f32, pos_y as f32), &font, &paint);
    let (level_w, _) = font.measure_str(level_text, Some(&paint));

    let font = FontBuilder::build(800, Slant::Italic, font_data, 35.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let pos_x = (INFO_PAD + 40) as f32 + level_w;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 46;
    card.canvas
        .draw_str(level_value, (pos_x, pos_y as f32), &font, &paint);
    let (value_w, _) = font.measure_str(level_value, Some(&paint));

    // Bar
    let rect_w = (W - 2 * INFO_PAD - 86) as f32 - level_w - value_w;
    let rect = Rect::new(0.0, 0.0, rect_w, 3.0);
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(51).build();
    let translate_x = (INFO_PAD + 54) as f32 + level_w + value_w;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 33;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * percent, 9.0);
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let translate_x = (INFO_PAD + 54) as f32 + level_w + value_w;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 30;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 9.0, 9.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    Ok(())
}

fn draw_medals(
    card: &mut CardBuilder<'_>,
    curr_medals: u32,
    total_medals: u32,
    font_data: &FontData,
) -> Result<(), InfoError> {
    enum MedalClub {
        C95,
        C90,
        C80,
        C60,
        C40,
        None,
    }

    impl MedalClub {
        fn from_percent(percent: f32) -> Self {
            if percent >= 0.95 {
                Self::C95
            } else if percent >= 0.9 {
                Self::C90
            } else if percent >= 0.8 {
                Self::C80
            } else if percent >= 0.6 {
                Self::C60
            } else if percent >= 0.4 {
                Self::C40
            } else {
                Self::None
            }
        }

        fn rgb(self) -> (u8, u8, u8) {
            match self {
                Self::C95 => (93, 89, 249),
                Self::C90 => (106, 237, 255),
                Self::C80 => (182, 106, 237),
                Self::C60 => (250, 89, 111),
                Self::C40 => (255, 140, 104),
                Self::None => (161, 190, 206),
            }
        }

        fn paint(self) -> PaintBuilder {
            let (r, g, b) = self.rgb();

            PaintBuilder::rgb(r, g, b)
        }
    }

    let percent = curr_medals as f32 / total_medals as f32;

    // Text left
    let font = FontBuilder::build(300, Slant::Italic, font_data, 35.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(168).build();
    let pos_x = INFO_PAD + 32;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 87;
    let medal_text = "Medals";
    card.canvas
        .draw_str(medal_text, (pos_x as f32, pos_y as f32), &font, &paint);
    let (medal_w, _) = font.measure_str(medal_text, Some(&paint));

    let font = FontBuilder::build(800, Slant::Italic, font_data, 35.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(168).build(); // simulating brightness
    let pos_x = (INFO_PAD + 40) as f32 + medal_w;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 87;
    let medal_percent_str = format!("{}%", (percent * 100.0) as u32);
    card.canvas
        .draw_str(&medal_percent_str, (pos_x, pos_y as f32), &font, &paint);

    let font = FontBuilder::build(800, Slant::Italic, font_data, 35.0)?;
    let (r, g, b) = MedalClub::from_percent(percent).rgb();
    let paint = PaintBuilder::rgb(r, g, b).alpha(168).build();
    let pos_x = (INFO_PAD + 40) as f32 + medal_w;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 87;
    card.canvas
        .draw_str(&medal_percent_str, (pos_x, pos_y as f32), &font, &paint);
    let (percent_w, _) = font.measure_str(&medal_percent_str, Some(&paint));

    // Text right
    let font = FontBuilder::build(400, Slant::Upright, font_data, 30.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).alpha(168).build();
    let pos_x = W - (INFO_PAD + 32);
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 86;
    let total_medals_str = format!("/{total_medals}");

    card.canvas.draw_str_align(
        &total_medals_str,
        (pos_x as f32, pos_y as f32),
        &font,
        &paint,
        Align::Right,
    );
    let (total_medals_w, _) = font.measure_str(total_medals_str, Some(&paint));

    let font = FontBuilder::build(500, Slant::Upright, font_data, 30.0)?;
    let paint = PaintBuilder::rgb(255, 255, 255).build();
    let pos_x = (W - (INFO_PAD + 32)) as f32 - total_medals_w;
    let pos_y = HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 86;
    let medals_str = card.int_buf.format(curr_medals);

    card.canvas.draw_str_align(
        medals_str,
        (pos_x, pos_y as f32),
        &font,
        &paint,
        Align::Right,
    );

    let (medals_w, _) = font.measure_str(medals_str, Some(&paint));

    // Split thin bars
    let rect_w = (W - 2 * INFO_PAD - 98) as f32 - medal_w - percent_w - total_medals_w - medals_w;
    let rect = Rect::new(0.0, 0.0, rect_w * 0.41, 3.0);
    let paint = MedalClub::None.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * 0.21, 3.0);
    let paint = MedalClub::C40.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w + rect_w * 0.4;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * 0.21, 3.0);
    let paint = MedalClub::C60.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w + rect_w * 0.6;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * 0.11, 3.0);
    let paint = MedalClub::C80.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w + rect_w * 0.8;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * 0.06, 3.0);
    let paint = MedalClub::C90.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w + rect_w * 0.9;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    let rect = Rect::new(0.0, 0.0, rect_w * 0.05, 3.0);
    let paint = MedalClub::C95.paint().build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w + rect_w * 0.95;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 73;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 3.0, 3.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    // Shadow
    let shadow_w = 6.0;
    let rect = Rect::new(0.0, 0.0, rect_w * percent + 2.0 * shadow_w, 20.0);
    let paint = PaintBuilder::rgb(r, g, b)
        .alpha(128)
        .mask_filter(BlurStyle::Normal, shadow_w)?
        .build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w - shadow_w;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 65;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 9.0, 9.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    // Thick bar
    let rect = Rect::new(0.0, 0.0, rect_w * percent, 9.0);
    let paint = PaintBuilder::rgb(r, g, b).build();
    let translate_x = (INFO_PAD + 53) as f32 + medal_w + percent_w;
    let translate_y =
        HEADER_H + INFO_PAD + INFO_UPPER_H + INFO_LOWER_MARGIN + INFO_LOWER_MARGIN + 70;

    card.canvas
        .translate((translate_x, translate_y as f32))
        .draw_round_rect(rect, 9.0, 9.0, &paint)
        .translate((-translate_x, -translate_y as f32));

    Ok(())
}
