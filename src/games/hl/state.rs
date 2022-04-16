use std::{cmp::Ordering, mem, sync::Arc};

use eyre::Report;
use image::{png::PngEncoder, ColorType, GenericImageView, ImageBuffer};
use rand::Rng;
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::{self, Receiver};
use twilight_model::{
    channel::embed::{Embed, EmbedField},
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker},
        Id,
    },
};

use crate::{
    core::{Context, CONFIG},
    error::InvalidGameState,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        Authored, ChannelExt,
    },
    BotResult,
};

use super::{GameStateInfo, HlGuess, HlVersion};

const W: u32 = 900;
const H: u32 = 250;
const ALPHA_THRESHOLD: u8 = 20;

pub struct GameState {
    pub previous: GameStateInfo,
    pub next: GameStateInfo,
    pub id: Id<MessageMarker>,
    pub channel: Id<ChannelMarker>,
    pub guild: Option<Id<GuildMarker>>,
    pub version: HlVersion,
    pub current_score: u32,
    pub highscore: u32,
    image_url_rx: Receiver<String>,
}

impl GameState {
    /// Be sure this is only called once after [`GameState::new`] / [`GameState::next`].
    /// Otherwise it will panic.
    pub async fn image(&mut self) -> String {
        match (&mut self.image_url_rx).await {
            Ok(url) => url,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to receive image url");
                warn!("{report:?}");

                String::new()
            }
        }
    }

    pub async fn new(
        ctx: &Context,
        origin: &(dyn Authored + Sync),
        highscore: u32,
    ) -> BotResult<Self> {
        let (previous, mut next) =
            tokio::try_join!(random_play(&ctx, 0.0, 0), random_play(&ctx, 0.0, 0))?;

        while next == previous {
            next = random_play(&ctx, 0.0, 0).await?;
        }

        debug!("{}pp vs {}pp", previous.pp, next.pp);

        let (tx, image_url_rx) = oneshot::channel();

        let pfp1 = &previous.avatar;
        let cover1 = &previous.cover;

        let pfp2 = &next.avatar;
        let cover2 = &next.cover;

        let url = match create_image(ctx, pfp1, pfp2, cover1, cover2).await {
            Ok(url) => url,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to create image");
                warn!("{report:?}");

                String::new()
            }
        };

        let _ = tx.send(url);

        Ok(Self {
            previous,
            next,
            id: Id::new(1),
            channel: origin.channel_id(),
            guild: origin.guild_id(),
            version: HlVersion::ScorePp,
            current_score: 0,
            highscore,
            image_url_rx,
        })
    }

    /// Set `next` to `previous` and get a new state info for `next`
    pub async fn next(&mut self, ctx: Arc<Context>) -> BotResult<()> {
        mem::swap(&mut self.previous, &mut self.next);

        self.next = random_play(&ctx, self.previous.pp, self.current_score).await?;

        while self.next == self.previous {
            self.next = random_play(&ctx, self.previous.pp, self.current_score).await?;
        }

        debug!("{}pp vs {}pp", self.previous.pp, self.next.pp);

        let pfp1 = mem::take(&mut self.previous.avatar);
        let cover1 = mem::take(&mut self.previous.cover);

        // Clone these since they're needed in the next round
        let pfp2 = self.next.avatar.clone();
        let cover2 = self.next.cover.clone();

        let (tx, image_url_rx) = oneshot::channel();
        self.image_url_rx = image_url_rx;

        // Create the image in the background so it's available when needed later
        tokio::spawn(async move {
            let url = match create_image(&ctx, &pfp1, &pfp2, &cover1, &cover2).await {
                Ok(url) => url,
                Err(err) => {
                    let report = Report::new(err).wrap_err("failed to create image");
                    warn!("{report:?}");

                    String::new()
                }
            };

            let _ = tx.send(url);
        });

        Ok(())
    }

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

        EmbedBuilder::new()
            .title(title)
            .fields(fields)
            .image(image)
            .footer(self.footer())
            .build()
    }

    pub fn footer(&self) -> String {
        let Self {
            current_score,
            highscore,
            ..
        } = self;

        format!("Current score: {current_score} • Highscore: {highscore}")
    }

    pub(super) fn check_guess(&self, guess: HlGuess) -> bool {
        match guess {
            HlGuess::Higher => self.next.pp >= self.previous.pp,
            HlGuess::Lower => self.next.pp <= self.previous.pp,
        }
    }
}

async fn create_image(
    ctx: &Context,
    pfp1: &str,
    pfp2: &str,
    cover1: &str,
    cover2: &str,
) -> BotResult<String> {
    // Gather the profile pictures and map covers
    let client = ctx.client();

    let (pfp_left, pfp_right, bg_left, bg_right) = tokio::try_join!(
        client.get_avatar(pfp1),
        client.get_avatar(pfp2),
        client.get_mapset_cover(cover1),
        client.get_mapset_cover(cover2),
    )?;

    let pfp_left = image::load_from_memory(&pfp_left)?.thumbnail(128, 128);
    let pfp_right = image::load_from_memory(&pfp_right)?.thumbnail(128, 128);
    let bg_left = image::load_from_memory(&bg_left)?;
    let bg_right = image::load_from_memory(&bg_right)?;

    // Combine the images
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

    // Encode the combined images
    let mut png_bytes: Vec<u8> = Vec::with_capacity((W * H * 4) as usize);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(blipped.as_raw(), W, H, ColorType::Rgba8)?;

    // Send image into discord channel
    let builder = MessageBuilder::new().attachment("higherlower.png", png_bytes);

    let mut message = CONFIG
        .get()
        .unwrap()
        .hl_channel
        .create_message(ctx, &builder)
        .await?
        .model()
        .await?;

    // Return the url to the message's image
    let attachment = message
        .attachments
        .pop()
        .ok_or(InvalidGameState::MissingAttachment)?;

    Ok(attachment.url)
}

async fn random_play(ctx: &Context, prev_pp: f32, curr_score: u32) -> BotResult<GameStateInfo> {
    let max_play = 25 - curr_score.min(24);
    let min_play = 24 - 2 * curr_score.min(12);

    let (rank, play): (u32, u32) = {
        let mut rng = rand::thread_rng();

        (rng.gen_range(1..=5000), rng.gen_range(min_play..max_play))
    };

    let page = ((rank - 1) / 50) + 1;
    let idx = (rank - 1) % 50;

    let player = ctx
        .osu()
        .performance_rankings(GameMode::STD)
        .page(page)
        .await?
        .ranking
        .swap_remove(idx as usize);

    let mut plays = ctx
        .osu()
        .user_scores(player.user_id)
        .limit(100)
        .mode(GameMode::STD)
        .best()
        .await?;

    plays.sort_unstable_by(|a, b| {
        let a_pp = (a.pp.unwrap_or(0.0) - prev_pp).abs();
        let b_pp = (b.pp.unwrap_or(0.0) - prev_pp).abs();

        a_pp.partial_cmp(&b_pp).unwrap_or(Ordering::Equal)
    });

    let play = plays.swap_remove(play as usize);

    let map_id = play.map.as_ref().unwrap().map_id;

    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(err) => return Err(err.into()),
        },
    };

    Ok(GameStateInfo::new(player, map, play))
}
