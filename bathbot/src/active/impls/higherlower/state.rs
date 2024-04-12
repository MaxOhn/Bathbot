use std::{mem, sync::Arc};

use bathbot_model::HlVersion;
use bathbot_util::{EmbedBuilder, MessageBuilder};
use eyre::{ContextCompat, Result, WrapErr};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::{self, Receiver};

use super::{
    farm_maps::{FarmEntries, FarmMap},
    score_pp::ScorePp,
    HlGuess,
};
use crate::{
    core::{BotConfig, Context, ContextExt},
    util::ChannelExt,
};

pub(super) const W: u32 = 900;
pub(super) const H: u32 = 250;

pub(super) enum ButtonState {
    HigherLower,
    Next {
        image: Option<Box<str>>,
        last_guess: HlGuess,
    },
    TryAgain {
        image: Option<Box<str>>,
        last_guess: HlGuess,
    },
}

// seems to be a false alarm by clippy
#[allow(clippy::large_enum_variant)]
pub(super) enum HigherLowerState {
    ScorePp {
        mode: GameMode,
        previous: ScorePp,
        next: ScorePp,
    },
    FarmMaps {
        entries: FarmEntries,
        previous: FarmMap,
        next: FarmMap,
    },
}

impl HigherLowerState {
    pub(super) async fn start_score_pp(
        ctx: Arc<Context>,
        mode: GameMode,
    ) -> Result<(Self, Receiver<String>)> {
        let (previous, mut next) = tokio::try_join!(
            ScorePp::random(ctx.cloned(), mode, None, 0),
            ScorePp::random(ctx.cloned(), mode, None, 0)
        )
        .wrap_err("Failed to create score pp entry")?;

        while next == previous {
            next = ScorePp::random(ctx.cloned(), mode, None, 0)
                .await
                .wrap_err("Failed to create score pp entry")?;
        }

        ScorePp::log(&previous, &next);

        let (tx, rx) = oneshot::channel();

        let pfp1 = previous.avatar_url.as_ref();
        let pfp2 = next.avatar_url.as_ref();

        let mapset_id1 = previous.mapset_id;
        let mapset_id2 = next.mapset_id;

        let url = match ScorePp::image(&ctx, pfp1, pfp2, mapset_id1, mapset_id2).await {
            Ok(url) => url,
            Err(err) => {
                warn!(?err, "Failed to create image");

                String::new()
            }
        };

        let _ = tx.send(url);

        let inner = Self::ScorePp {
            mode,
            previous,
            next,
        };

        Ok((inner, rx))
    }

    pub(super) async fn start_farm_maps(
        ctx: Arc<Context>,
        entries: FarmEntries,
    ) -> Result<(Self, Receiver<String>)> {
        let previous = FarmMap::random(ctx.cloned(), &entries, None, 0)
            .await
            .wrap_err("Failed to create farm map entry")?;

        let next = FarmMap::random(ctx.cloned(), &entries, Some(&previous), 0)
            .await
            .wrap_err("Failed to create farm map entry")?;

        FarmMap::log(&previous, &next);

        let (tx, rx) = oneshot::channel();

        let url = match FarmMap::image(&ctx, previous.mapset_id, next.mapset_id).await {
            Ok(url) => url,
            Err(err) => {
                warn!(?err, "Failed to create image");

                String::new()
            }
        };

        let _ = tx.send(url);

        let inner = Self::FarmMaps {
            entries,
            previous,
            next,
        };

        Ok((inner, rx))
    }

    pub(super) async fn restart(&mut self, ctx: Arc<Context>) -> Result<(Self, Receiver<String>)> {
        match self {
            Self::ScorePp { mode, .. } => Self::start_score_pp(ctx, *mode).await,
            Self::FarmMaps { entries, .. } => Self::start_farm_maps(ctx, entries.to_owned()).await,
        }
    }

    pub(super) async fn next(
        &mut self,
        ctx: Arc<Context>,
        curr_score: u32,
    ) -> Result<Receiver<String>> {
        let rx = match self {
            Self::ScorePp {
                mode,
                previous,
                next,
            } => {
                let mode = *mode;
                mem::swap(previous, next);

                *next = ScorePp::random(ctx.cloned(), mode, Some(&*previous), curr_score)
                    .await
                    .wrap_err("Failed to create score pp entry")?;

                while previous == next {
                    *next = ScorePp::random(ctx.cloned(), mode, Some(&*previous), curr_score)
                        .await
                        .wrap_err("Failed to create score pp entry")?;
                }

                ScorePp::log(&*previous, &*next);

                let pfp1 = mem::take(&mut previous.avatar_url);

                // Clone this since it's needed in the next round
                let pfp2 = next.avatar_url.clone();

                let mapset_id1 = previous.mapset_id;
                let mapset_id2 = next.mapset_id;

                let (tx, rx) = oneshot::channel();

                // Create the image in the background so it's available when needed later
                tokio::spawn(async move {
                    let url = match ScorePp::image(&ctx, &pfp1, &pfp2, mapset_id1, mapset_id2).await
                    {
                        Ok(url) => url,
                        Err(err) => {
                            warn!(?err, "Failed to create image");

                            String::new()
                        }
                    };

                    let _ = tx.send(url);
                });

                rx
            }
            Self::FarmMaps {
                entries,
                previous,
                next,
            } => {
                mem::swap(previous, next);
                *next = FarmMap::random(ctx.cloned(), entries, Some(&*previous), curr_score)
                    .await
                    .wrap_err("Failed to create farm map entry")?;

                FarmMap::log(&*previous, &*next);

                let mapset1 = previous.mapset_id;
                let mapset2 = next.mapset_id;

                let (tx, rx) = oneshot::channel();

                // Create the image in the background so it's available when needed later
                tokio::spawn(async move {
                    let url = match FarmMap::image(&ctx, mapset1, mapset2).await {
                        Ok(url) => url,
                        Err(err) => {
                            warn!(?err, "Failed to create image");

                            String::new()
                        }
                    };

                    let _ = tx.send(url);
                });

                rx
            }
        };

        Ok(rx)
    }

    pub(super) async fn upload_image(ctx: &Context, img: &[u8], content: String) -> Result<String> {
        // Encode the combined images
        let mut png_bytes: Vec<u8> = Vec::with_capacity((W * H * 4) as usize);

        PngEncoder::new(&mut png_bytes)
            .write_image(img, W, H, ColorType::Rgba8)
            .wrap_err("Failed to encode image")?;

        // Send image into discord channel
        let builder = MessageBuilder::new()
            .attachment("higherlower.png", png_bytes)
            .content(content);

        let mut message = BotConfig::get()
            .hl_channel
            .create_message(ctx, builder, None)
            .await?
            .model()
            .await
            .wrap_err("Failed to create message")?;

        // Return the url to the message's image
        let attachment = message.attachments.pop().wrap_err("Missing attachment")?;

        Ok(attachment.url)
    }

    pub(super) fn to_embed(&self, revealed: bool) -> EmbedBuilder {
        let mut title = "Higher or Lower: ".to_owned();

        let builder = match self {
            HigherLowerState::ScorePp {
                mode,
                previous,
                next,
            } => {
                title.push_str("Score PP");

                match mode {
                    GameMode::Osu => {}
                    GameMode::Taiko => title.push_str(" (taiko)"),
                    GameMode::Catch => title.push_str(" (ctb)"),
                    GameMode::Mania => title.push_str(" (mania)"),
                }

                ScorePp::to_embed(previous, next, revealed)
            }
            HigherLowerState::FarmMaps {
                entries: _,
                previous,
                next,
            } => {
                title.push_str("Farm maps");

                FarmMap::to_embed(previous, next, revealed)
            }
        };

        builder.title(title)
    }

    pub(super) fn check_guess(&self, guess: HlGuess) -> bool {
        match self {
            Self::ScorePp { previous, next, .. } => match guess {
                HlGuess::Higher => next.pp >= previous.pp,
                HlGuess::Lower => next.pp <= previous.pp,
            },
            Self::FarmMaps { previous, next, .. } => match guess {
                HlGuess::Higher => next.farm >= previous.farm,
                HlGuess::Lower => next.farm <= previous.farm,
            },
        }
    }

    pub(super) fn version(&self) -> HlVersion {
        match self {
            Self::ScorePp { .. } => HlVersion::ScorePp,
            Self::FarmMaps { .. } => HlVersion::FarmMaps,
        }
    }
}

pub(super) fn mapset_cover(mapset_id: u32) -> String {
    format!("https://assets.ppy.sh/beatmaps/{mapset_id}/covers/cover.jpg")
}
