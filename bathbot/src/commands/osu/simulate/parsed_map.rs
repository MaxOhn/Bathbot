use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_pp::{Beatmap, Difficulty};
use rosu_v2::prelude::GameMode;
use twilight_model::channel::Attachment;

use crate::core::{Context, commands::CommandOrigin};

pub struct AttachedSimulateMap {
    pub pp_map: Beatmap,
    pub max_combo: u32,
    pub filename: Box<str>,
}

impl AttachedSimulateMap {
    pub async fn new(
        orig: &CommandOrigin<'_>,
        attachment: Box<Attachment>,
        mode: Option<GameMode>,
    ) -> Result<Option<Self>> {
        if !attachment.filename.ends_with(".osu") {
            let content = "The attached file must be of type .osu";
            orig.error(content).await?;

            return Ok(None);
        }

        let bytes = match Context::client().get_discord_attachment(&attachment).await {
            Ok(bytes) => bytes,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("Failed to download attachment"));
            }
        };

        let mut pp_map = match Beatmap::from_bytes(&bytes) {
            Ok(map) => map,
            Err(err) => {
                debug!(err = ?Report::new(err), "Failed to parse attachment as beatmap");

                let content = "Failed to parse file. Be sure you provide a valid .osu file.";
                orig.error(content).await?;

                return Ok(None);
            }
        };

        if let Some(mode) = mode {
            // TODO: use mods
            let _ = pp_map.convert_mut((mode as u8).into(), &Default::default());
        }

        let max_combo = if pp_map.check_suspicion().is_ok() {
            Difficulty::new().calculate(&pp_map).max_combo()
        } else {
            0
        };

        Ok(Some(Self {
            pp_map,
            max_combo,
            filename: attachment.filename.into(),
        }))
    }
}
