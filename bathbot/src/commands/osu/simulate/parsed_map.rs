use std::borrow::Cow;

use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_pp::{Beatmap, BeatmapExt, GameMode as Mode};
use rosu_v2::prelude::GameMode;
use twilight_model::channel::Attachment;

use crate::core::{commands::CommandOrigin, Context};

pub struct AttachedSimulateMap {
    pub pp_map: Beatmap,
    pub max_combo: u32,
    pub is_convert: bool,
    pub filename: Box<str>,
}

impl AttachedSimulateMap {
    pub async fn new(
        ctx: &Context,
        orig: &CommandOrigin<'_>,
        attachment: Box<Attachment>,
        mode: Option<GameMode>,
    ) -> Result<Option<Self>> {
        if !attachment.filename.ends_with(".osu") {
            let content = "The attached file must be of type .osu";
            orig.error(ctx, content).await?;

            return Ok(None);
        }

        let bytes = match ctx.client().get_discord_attachment(&attachment).await {
            Ok(bytes) => bytes,
            Err(err) => {
                let _ = orig.error(ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to download attachment"));
            }
        };

        let mut pp_map = match Beatmap::from_bytes(&bytes).await {
            Ok(map) => map,
            Err(err) => {
                debug!(err = ?Report::new(err), "Failed to parse attachment as beatmap");

                let content = "Failed to parse file. Be sure you provide a valid .osu file.";
                orig.error(ctx, content).await?;

                return Ok(None);
            }
        };

        let mut is_convert = false;

        if let Some(mode) = mode {
            let mode = match mode {
                GameMode::Osu => Mode::Osu,
                GameMode::Taiko => Mode::Taiko,
                GameMode::Catch => Mode::Catch,
                GameMode::Mania => Mode::Mania,
            };

            if let Cow::Owned(map) = pp_map.convert_mode(mode) {
                pp_map = map;
                is_convert = true;
            } else if mode == Mode::Catch && pp_map.mode != Mode::Catch {
                pp_map.mode = mode;
                is_convert = true;
            }
        }

        let max_combo = pp_map.stars().calculate().max_combo() as u32;

        Ok(Some(Self {
            pp_map,
            max_combo,
            is_convert,
            filename: attachment.filename.into(),
        }))
    }
}
