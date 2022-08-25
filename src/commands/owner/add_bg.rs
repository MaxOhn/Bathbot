use std::{str::FromStr, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{BeatmapsetCompact, GameMode, OsuError};
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
};

use crate::{
    core::BotConfig,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE},
        interaction::InteractionCommand,
        InteractionCommandExt,
    },
    BotResult, Context,
};

use super::OwnerAddBg;

pub async fn addbg(
    ctx: Arc<Context>,
    command: InteractionCommand,
    bg: OwnerAddBg,
) -> BotResult<()> {
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

                    return Err(err.into());
                }
            };

            // Store in file
            if let Err(err) = file.write_all(&content).await {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.into());
            }
            path
        }
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    // Check if valid mapset id
    let content = match prepare_mapset(&ctx, mapset_id, &image.filename, mode).await {
        Ok(mapset) => format!(
            "Background for [{artist} - {title}]({base}s/{id}) successfully added ({mode})",
            artist = mapset.artist,
            title = mapset.title,
            base = OSU_BASE,
            id = mapset_id,
            mode = mode
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
    ctx: &Context,
    mapset_id: u32,
    filename: &str,
    mode: GameMode,
) -> Result<BeatmapsetCompact, &'static str> {
    let db_fut = ctx.psql().get_beatmapset::<BeatmapsetCompact>(mapset_id);

    let mapset = match db_fut.await {
        Ok(mapset) => mapset,
        Err(_) => match ctx.osu().beatmapset(mapset_id).await {
            Ok(mapset) => {
                if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                    warn!("{:?}", Report::new(err));
                }

                mapset.into()
            }
            Err(OsuError::NotFound) => {
                return Err("No mapset found with the name of the given file as id")
            }
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to request mapset");
                error!("{:?}", report);

                return Err(OSU_API_ISSUE);
            }
        },
    };

    if let Err(err) = ctx.psql().add_tag_mapset(mapset_id, filename, mode).await {
        let report = Report::new(err).wrap_err("error while adding mapset to tags table");
        warn!("{:?}", report);

        return Err("There is already an entry with this mapset id");
    }

    Ok(mapset)
}
