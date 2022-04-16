use image::{png::PngEncoder, ColorType, GenericImageView, ImageBuffer};
use twilight_model::{
    channel::embed::{Embed, EmbedField},
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::{Context, CONFIG},
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        ChannelExt,
    },
    BotResult,
};

use super::{GameStateInfo, HlGuess};

const W: u32 = 900;
const H: u32 = 250;
const ALPHA_THRESHOLD: u8 = 20;

pub struct GameState {
    pub previous: GameStateInfo,
    pub next: GameStateInfo,
    #[allow(unused)] // TODO
    pub player: Id<UserMarker>,
    pub id: Id<MessageMarker>,
    pub channel: Id<ChannelMarker>,
    pub guild: Option<Id<GuildMarker>>,
    pub mode: u8,
    pub current_score: u32,
    pub highscore: u32,
}

impl GameState {
    pub fn to_embed(&self, image: String) -> Embed {
        let title = "Higher or Lower: PP";

        let fields = vec![
            EmbedField {
                inline: false,
                name: format!("__Previous:__ {}", self.previous.player_string),
                value: self.previous.play_string(true),
            },
            EmbedField {
                inline: false,
                name: format!("__Next:__ {}", self.next.player_string),
                value: self.next.play_string(false),
            },
        ];

        let footer = format!(
            "Current score: {} • Highscore: {}",
            self.current_score, self.highscore
        );

        EmbedBuilder::new()
            .title(title)
            .fields(fields)
            .image(image)
            .footer(footer)
            .build()
    }

    pub(super) fn check_guess(&self, guess: HlGuess) -> bool {
        match guess {
            HlGuess::Higher => self.next.pp >= self.previous.pp,
            HlGuess::Lower => self.next.pp <= self.previous.pp,
        }
    }

    pub async fn create_image(&self, ctx: &Context) -> BotResult<String> {
        let client = ctx.client();

        let (pfp_left, pfp_right, bg_left, bg_right) = tokio::try_join!(
            client.get_avatar(&self.previous.avatar),
            client.get_avatar(&self.next.avatar),
            client.get_mapset_cover(&self.previous.cover),
            client.get_mapset_cover(&self.next.cover)
        )?;

        let pfp_left = image::load_from_memory(&pfp_left)?.thumbnail(128, 128);
        let pfp_right = image::load_from_memory(&pfp_right)?.thumbnail(128, 128);
        let bg_left = image::load_from_memory(&bg_left)?;
        let bg_right = image::load_from_memory(&bg_right)?;

        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        for (x, y, pixel) in pfp_left.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(x, y, pixel);
            }
        }

        let pfp_right_width = pfp_right.width();

        for (x, y, pixel) in pfp_right.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(W - pfp_right_width + x, y, pixel);
            }
        }

        let mut png_bytes: Vec<u8> = Vec::with_capacity((W * H * 4) as usize);
        let png_encoder = PngEncoder::new(&mut png_bytes);
        png_encoder.encode(blipped.as_raw(), W, H, ColorType::Rgba8)?;

        let builder = MessageBuilder::new().attachment("higherlower.png", png_bytes);

        let mut message = CONFIG
            .get()
            .unwrap()
            .hl_channel
            .create_message(ctx, &builder)
            .await?
            .model()
            .await?;

        Ok(message.attachments.pop().unwrap().url)
    }
}
