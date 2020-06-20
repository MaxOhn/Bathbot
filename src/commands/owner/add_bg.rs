use crate::{
    util::{globals::OSU_API_ISSUE, MessageExt},
    MySQL, Osu,
};

use rosu::{backend::BeatmapRequest, models::GameMode};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{env, path::PathBuf, str::FromStr};
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
};

#[command]
#[description = "Add background for the background game"]
#[aliases("bgadd")]
async fn addbg(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Check if msg has attachement
    if msg.attachments.is_empty() {
        msg.channel_id
            .say(
                ctx,
                "You must attach an image to the command that has the mapset id as name",
            )
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    }
    // Parse arguments as mode
    let mode = match args.single::<String>() {
        Ok(s) => match s.to_lowercase().as_str() {
            "mna" | "mania" | "m" => GameMode::MNA,
            "osu" | "std" | "standard" | "o" => GameMode::STD,
            _ => {
                msg.channel_id
                    .say(
                        ctx,
                        "Could not parse first argument as mode. \
                        Provide either `mna`, or `std`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        },
        Err(_) => GameMode::STD,
    };
    let attachement = msg.attachments.first().unwrap().clone();
    // Check if attachement as proper name
    let mut filename_split = attachement.filename.split('.');
    let mapset_id = match filename_split.next() {
        Some(name) => match u32::from_str(name) {
            Ok(id) => id,
            Err(_) => {
                msg.channel_id
                    .say(
                        ctx,
                        "Provided image has no appropriate name. \
                        Be sure to let the name be the mapset id, e.g. 948199.png",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        },
        None => {
            msg.channel_id
                .say(
                    ctx,
                    "Provided image has no appropriate name. \
                    Be sure to let the name be the mapset id, e.g. 948199.png",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
    };
    // Check if attachement has proper file type
    let filetype = match filename_split.next() {
        Some(filetype) if filetype == "jpg" || filetype == "jpeg" || filetype == "png" => filetype,
        _ => {
            msg.channel_id
                .say(
                    ctx,
                    "Provided image has no appropriate file type. \
                    It must be either `.jpg`, `.jpeg`, or `.png`",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
    };
    // Download attachement
    let path = match attachement.download().await {
        Ok(content) => {
            let mut path = PathBuf::from(env::var("BG_PATH").unwrap());
            match mode {
                GameMode::STD => path.push("osu"),
                GameMode::MNA => path.push("mania"),
                GameMode::TKO | GameMode::CTB => unreachable!(),
            }
            path.push(&attachement.filename);
            // Create file
            let mut file = match File::create(&path).await {
                Ok(file) => file,
                Err(why) => {
                    msg.channel_id
                        .say(ctx, "Error while creating file")
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
                }
            };
            // Store in file
            if let Err(why) = file.write_all(&content).await {
                msg.channel_id
                    .say(ctx, "Error while writing to file")
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
            path
        }
        Err(why) => {
            msg.channel_id
                .say(ctx, "Error while downloading image")
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(why.to_string().into());
        }
    };
    // Check if valid mapset id
    let content = match prepare_mapset(ctx, mapset_id, filetype, mode).await {
        Ok(_) => "Background successfully added",
        Err(err_msg) => {
            let _ = remove_file(path).await;
            err_msg
        }
    };
    msg.channel_id
        .say(ctx, content)
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

async fn prepare_mapset(
    ctx: &Context,
    mapset_id: u32,
    filetype: &str,
    mode: GameMode,
) -> Result<(), &'static str> {
    let data = ctx.data.read().await;
    let mysql = data.get::<MySQL>().unwrap();
    if mysql.get_beatmapset(mapset_id).await.is_err() {
        let osu = data.get::<Osu>().unwrap();
        let req = BeatmapRequest::new().mapset_id(mapset_id);
        match req.queue(osu).await {
            Ok(maps) => {
                if maps.is_empty() {
                    return Err("No mapset found with the name of the given file as id");
                }
            }
            Err(why) => {
                error!("Osu api issue: {}", why);
                return Err(OSU_API_ISSUE);
            }
        }
    }
    if let Err(why) = mysql.add_tag_mapset(mapset_id, filetype, mode).await {
        error!("Error while adding mapset to tags table: {}", why);
        return Err("Some database issue, blame bade");
    }
    Ok(())
}
