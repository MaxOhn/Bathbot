use std::{fmt::Write, mem, sync::Arc};

use eyre::Report;
use image::{png::PngEncoder, ColorType};
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::{self, Receiver};
use twilight_model::channel::embed::{Embed, EmbedField};

use crate::{
    core::{Context, CONFIG},
    error::InvalidGameState,
    games::hl::score_pp::ScorePp,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        datetime::sec_to_minsec,
        numbers::round,
        ChannelExt,
    },
    BotResult,
};

use super::{
    farm_map::{FarmEntries, FarmMap},
    HlGuess, HlVersion, H, W,
};

pub(super) enum GameStateKind {
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

impl GameStateKind {
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

    pub async fn restart(self, ctx: &Context) -> BotResult<(Self, Receiver<String>)> {
        match self {
            Self::ScorePp { mode, .. } => Self::score_pp(ctx, mode).await,
            Self::FarmMaps { entries, .. } => Self::farm_maps(ctx, entries).await,
        }
    }

    pub async fn next(
        &mut self,
        ctx: Arc<Context>,
        curr_score: u32,
    ) -> BotResult<Receiver<String>> {
        let rx = match self {
            Self::ScorePp {
                mode,
                previous,
                next,
            } => {
                let mode = *mode;
                mem::swap(previous, next);

                *next = ScorePp::random(&ctx, mode, previous.pp, curr_score).await?;

                while previous == next {
                    *next = ScorePp::random(&ctx, mode, previous.pp, curr_score).await?;
                }

                debug!("{}pp vs {}pp", previous.pp, next.pp);

                let pfp1 = mem::take(&mut previous.avatar_url);

                // Clone this since it's needed in the next round
                let pfp2 = next.avatar_url.clone();

                let mapset1 = previous.mapset_id;
                let mapset2 = next.mapset_id;

                let (tx, rx) = oneshot::channel();

                // Create the image in the background so it's available when needed later
                tokio::spawn(async move {
                    let url = match ScorePp::image(&ctx, &pfp1, &pfp2, mapset1, mapset2).await {
                        Ok(url) => url,
                        Err(err) => {
                            let report = Report::new(err).wrap_err("failed to create image");
                            warn!("{report:?}");

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
                *next = FarmMap::random(&ctx, entries, Some(previous.farm), curr_score).await?;

                debug!("farm: {} vs {}", previous.farm, next.farm);

                let mapset1 = previous.mapset_id;
                let mapset2 = next.mapset_id;

                let (tx, rx) = oneshot::channel();

                // Create the image in the background so it's available when needed later
                tokio::spawn(async move {
                    let url = match FarmMap::image(&ctx, mapset1, mapset2).await {
                        Ok(url) => url,
                        Err(err) => {
                            let report = Report::new(err).wrap_err("failed to create image");
                            warn!("{report:?}");

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

    pub async fn farm_maps(
        ctx: &Context,
        entries: FarmEntries,
    ) -> BotResult<(Self, Receiver<String>)> {
        let previous = FarmMap::random(ctx, &entries, None, 0).await?;
        let next = FarmMap::random(ctx, &entries, Some(previous.farm), 0).await?;

        debug!("farm: {} vs {}", previous.farm, next.farm);

        let (tx, rx) = oneshot::channel();

        let mapset1 = previous.mapset_id;
        let mapset2 = next.mapset_id;

        let url = match FarmMap::image(ctx, mapset1, mapset2).await {
            Ok(url) => url,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to create image");
                warn!("{report:?}");

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

    pub async fn score_pp(ctx: &Context, mode: GameMode) -> BotResult<(Self, Receiver<String>)> {
        let (previous, mut next) = tokio::try_join!(
            ScorePp::random(ctx, mode, 0.0, 0),
            ScorePp::random(ctx, mode, 0.0, 0)
        )?;

        while next == previous {
            next = ScorePp::random(ctx, mode, 0.0, 0).await?;
        }

        debug!("{}pp vs {}pp", previous.pp, next.pp);

        let (tx, rx) = oneshot::channel();

        let pfp1 = &previous.avatar_url;
        let mapset1 = previous.mapset_id;

        let pfp2 = &next.avatar_url;
        let mapset2 = next.mapset_id;

        let url = match ScorePp::image(ctx, pfp1, pfp2, mapset1, mapset2).await {
            Ok(url) => url,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to create image");
                warn!("{report:?}");

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

    pub fn to_embed(&self, image: String) -> EmbedBuilder {
        let mut title = "Higher or Lower: ".to_owned();

        let builder = match self {
            Self::ScorePp {
                mode,
                previous,
                next,
            } => {
                title.push_str("Score PP");

                match mode {
                    GameMode::STD => {}
                    GameMode::TKO => title.push_str(" (taiko)"),
                    GameMode::CTB => title.push_str(" (ctb)"),
                    GameMode::MNA => title.push_str(" (mania)"),
                }

                let fields = vec![
                    EmbedField {
                        inline: false,
                        name: format!("__Previous:__ {}", previous.player_string),
                        value: previous.play_string(true),
                    },
                    EmbedField {
                        inline: false,
                        name: format!("__Next:__ {}", next.player_string),
                        value: next.play_string(false),
                    },
                ];

                EmbedBuilder::new().fields(fields)
            }
            Self::FarmMaps { previous, next, .. } => {
                title.push_str("Farm maps");

                let description = format!(
                    "**__Previous:__ [{prev_map}]({prev_url})**\n\
                    `{prev_stars:.2}★` • `{prev_len}` • `{prev_combo}x` • Ranked <t:{prev_timestamp}:D>\n\
                    `CS {prev_cs}` `AR {prev_ar}` `OD {prev_od}` `HP {prev_hp}` • In **{farm}** top score{prev_plural}\n\
                    **__Next:__ [{next_map}]({next_url})**\n\
                    `{next_stars:.2}★` • `{next_len}` • `{next_combo}x` • Ranked <t:{next_timestamp}:D>\n\
                    `CS {next_cs}` `AR {next_ar}` `OD {next_od}` `HP {next_hp}` • In **???** top scores",
                    prev_map = previous.map_string,
                    prev_url = previous.map_url,
                    prev_stars = previous.stars,
                    prev_len = sec_to_minsec(previous.seconds_drain),
                    prev_combo = previous.combo,
                    prev_timestamp = previous.ranked.timestamp(),
                    prev_cs = previous.cs,
                    prev_ar = previous.ar,
                    prev_od = previous.od,
                    prev_hp = previous.hp,
                    farm = previous.farm,
                    prev_plural = if previous.farm == 1 { "" } else { "s" },
                    next_map = next.map_string,
                    next_url = next.map_url,
                    next_stars = next.stars,
                    next_len = sec_to_minsec(next.seconds_drain),
                    next_combo = next.combo,
                    next_timestamp = next.ranked.timestamp(),
                    next_cs = next.cs,
                    next_ar = next.ar,
                    next_od = next.od,
                    next_hp = next.hp,
                );

                EmbedBuilder::new().description(description)
            }
        };

        builder.title(title).image(image)
    }

    pub fn reveal(&self, embed: &mut Embed) {
        match self {
            Self::ScorePp { next, .. } => {
                if let Some(field) = embed.fields.last_mut() {
                    field.value.truncate(field.value.len() - 7);
                    let _ = write!(field.value, "__{}pp__**", round(next.pp));
                }
            }
            Self::FarmMaps { next, .. } => {
                if let Some(ref mut description) = embed.description {
                    description.truncate(description.len() - 16);
                    let _ = write!(
                        description,
                        "__{}__** top score{plural}",
                        next.farm,
                        plural = if next.farm == 1 { "" } else { "s" }
                    );
                }
            }
        }
    }

    pub fn version(&self) -> HlVersion {
        match self {
            Self::ScorePp { .. } => HlVersion::ScorePp,
            Self::FarmMaps { .. } => HlVersion::FarmMaps,
        }
    }

    pub async fn upload_image(ctx: &Context, img: &[u8], content: String) -> BotResult<String> {
        // Encode the combined images
        let mut png_bytes: Vec<u8> = Vec::with_capacity((W * H * 4) as usize);
        let png_encoder = PngEncoder::new(&mut png_bytes);
        png_encoder.encode(img, W, H, ColorType::Rgba8)?;

        // Send image into discord channel
        let builder = MessageBuilder::new()
            .attachment("higherlower.png", png_bytes)
            .content(content);

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
}
