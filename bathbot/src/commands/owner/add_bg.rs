use std::{str::FromStr, sync::Arc};

use bathbot_psql::model::osu::ArtistTitle;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_BASE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::GameMode;
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
};

use super::OwnerAddBg;
use crate::{
    core::{BotConfig, ContextExt},
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn addbg(ctx: Arc<Context>, command: InteractionCommand, bg: OwnerAddBg) -> Result<()> {
    let OwnerAddBg { image, mode } = bg;

    let mode = mode.map_or(GameMode::Osu, GameMode::from);

    // Check if attachement as proper name
    let mut filename_split = image.filename.split('.');

    let mapset_id = match filename_split.next().map(u32::from_str) {
        Some(Ok(id)) => id,
        None | Some(Err(_)) => {
            let content = "Provided image has no appropriate name. \
                Be sure to let the name be the mapset id, e.g. 948199.png";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    // Check if attachement has proper file type
    let valid_filetype_opt = filename_split
        .next()
        .filter(|&filetype| filetype == "jpg" || filetype == "png");

    if valid_filetype_opt.is_none() {
        let content = "Provided image has inappropriate type. Must be either `.jpg` or `.png`";
        command.error(&ctx, content).await?;

        return Ok(());
    }

    // Download attachement
    let path = match ctx.client().get_discord_attachment(&image).await {
        Ok(content) => {
            let mut path = BotConfig::get().paths.backgrounds.clone();

            match mode {
                GameMode::Osu => path.push("osu"),
                GameMode::Mania => path.push("mania"),
                GameMode::Taiko | GameMode::Catch => unreachable!(),
            }

            path.push(&image.filename);

            // Create file
            let mut file = match File::create(&path).await {
                Ok(file) => file,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;
                    let err = Report::new(err).wrap_err("failed to create file for new bg");

                    return Err(err);
                }
            };

            // Store in file
            if let Err(err) = file.write_all(&content).await {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;
                let err = Report::new(err).wrap_err("failed writing to bg file");

                return Err(err);
            }
            path
        }
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get discord attachment"));
        }
    };

    // Check if valid mapset id
    let content = match prepare_mapset(ctx.cloned(), mapset_id, &image.filename, mode).await {
        Ok(ArtistTitle { artist, title }) => format!(
            "Background for [{artist} - {title}]({OSU_BASE}s/{mapset_id}) successfully added ({mode})",
        ),
        Err(err_msg) => {
            let _ = remove_file(path).await;

            err_msg.to_owned()
        }
    };

    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}

async fn prepare_mapset(
    ctx: Arc<Context>,
    mapset_id: u32,
    filename: &str,
    mode: GameMode,
) -> Result<ArtistTitle, &'static str> {
    let artist_title = match ctx.osu_map().artist_title(mapset_id).await {
        Ok(artist_title) => artist_title,
        Err(err) => {
            warn!("{:?}", Report::new(err));

            return Err(GENERAL_ISSUE);
        }
    };

    let upsert_fut = ctx.games().bggame_upsert_mapset(mapset_id, filename, mode);

    if let Err(err) = upsert_fut.await {
        warn!("{err:?}");

        return Err("There is already an entry with this mapset id");
    }

    Ok(artist_title)
}
