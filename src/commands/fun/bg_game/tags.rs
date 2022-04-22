use std::{str::FromStr, sync::Arc, time::Duration};

use eyre::Report;
use rand::RngCore;
use rosu_v2::model::GameMode;
use tokio::fs;
use tokio_stream::StreamExt;
use twilight_model::{channel::ReactionType, gateway::event::Event, http::attachment::Attachment};

use super::ReactionWrapper;
use crate::{
    database::MapsetTagWrapper,
    games::bg::MapsetTags,
    util::{
        constants::{
            common_literals::{MANIA, OSU},
            GENERAL_ISSUE, OSU_BASE, OWNER_USER_ID,
        },
        send_reaction, CowUtils, Emote,
    },
    BotResult, Context, CONFIG,
};

#[command]
#[short_desc("Help tagging backgrounds by tagging them manually")]
#[long_desc(
    "Manage the tags of a background for the bg game.\n\
    First argument must be the mapset id, second argument must be either \
    `a` or `add` to add tags, or `r` or `remove` to remove them. \n\
    After that provide any of these pre-selected keywords:\n\
    `farm, streams, alternate, old, meme, hardname, easy, hard, tech, weeb, bluesky, english`\n\
    By default, all tags are marked as **true**, so removing them will be more important."
)]
#[usage("[mapset id] [add/a/remove/r] [list of tags]")]
#[example("21662 r hard farm streams alternate hardname tech weeb bluesky")]
#[aliases("bgtm", "bgtagmanual")]
#[owner()]
async fn bgtagsmanual(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    // Parse mapset id
    let mapset_id = match args.next().map(u32::from_str) {
        Some(Ok(num)) => num,
        Some(Err(_)) => {
            let content = "Could not parse mapset id. Be sure to specify it as first argument";

            return msg.error(&ctx, content).await;
        }
        None => {
            let content = "Arguments: `[mapset id] [add/a/remove/r] [list of tags]`\n\
            Example: `21662 r hard farm streams alternate hardname tech weeb bluesky`\n\
            Tags: `farm, streams, alternate, old, meme, hardname, easy, hard, tech, \
            weeb, bluesky, english`";

            let builder = MessageBuilder::new().content(content);
            msg.create_message(&ctx, builder).await?;

            return Ok(());
        }
    };

    // Check if there is background for the given mapset id
    if ctx.psql().get_tags_mapset(mapset_id).await.is_err() {
        let content = "No background entry found with this id";

        return msg.error(&ctx, content).await;
    }

    // Parse action
    let action = match args.next().map(Action::from_str) {
        Some(Ok(action)) => action,
        None | Some(Err(_)) => {
            let content = "Could not parse action. \
                Be sure to specify `r`, `remove`, `a`, or `add` as second argument";

            return msg.error(&ctx, content).await;
        }
    };

    // Parse tags
    let mut tags = MapsetTags::empty();

    while !args.is_empty() {
        match args.next().map(MapsetTags::from_str) {
            Some(Ok(tag)) => tags.insert(tag),
            Some(Err(tag)) => {
                let content = format!(
                    "Could not parse tag `{tag}`.\n\
                    Be sure to only give these tags:\n\
                    `farm, streams, alternate, old, meme, hardname, \
                    easy, hard, tech, weeb, bluesky, english`"
                );

                return msg.error(&ctx, content).await;
            }
            None => unreachable!(),
        }
    }

    let result = if tags.is_empty() {
        ctx.psql().get_tags_mapset(mapset_id).await
    } else {
        let db_result = match action {
            Action::Add => ctx.psql().add_tags_mapset(mapset_id, tags).await,
            Action::Remove => ctx.psql().remove_tags_mapset(mapset_id, tags).await,
        };

        match db_result {
            Ok(_) => ctx.psql().get_tags_mapset(mapset_id).await,
            Err(err) => Err(err),
        }
    };

    // Then show the final tags
    match result {
        Ok(tags) => {
            let content = format!("{OSU_BASE}beatmapsets/{mapset_id} is now tagged as:\n{tags}");

            let builder = MessageBuilder::new().content(content);
            msg.create_message(&ctx, builder).await?;
        }
        Err(err) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    }

    Ok(())
}

// #[command]
// #[short_desc("Help out tagging backgrounds")]
// #[long_desc(
//     "Let me give you mapsets that still need to be tagged.\n\
//     React to them properly, then lock it in by reacting with ✅.\n\
//     To leave the loop, react with ❌ or just wait 10 minutes.\n\
//     Mode can be specified in the first argument, defaults to std.\n\
//     **You need to be verified to use this command**, feel free to \
//     let Badewanne3 know if you want to help out tagging backgrounds."
// )]
// #[usage("[std / mna]")]
// #[aliases("bgt", "bgtag")]
// #[owner()]
async fn bgtags(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    // Parse arguments as mode
    let mode = match args.next() {
        Some(arg) => match arg.cow_to_ascii_lowercase().as_ref() {
            "mna" | "mania" | "m" => GameMode::MNA,
            "osu" | "std" | "standard" | "o" => GameMode::STD,
            _ => {
                let content = "Could not parse first argument as mode. \
                Provide either `mna`, or `std`";

                return msg.error(&ctx, content).await;
            }
        },
        None => GameMode::STD,
    };

    let mut untagged = match ctx.psql().get_all_tags_mapset(mode).await {
        Ok(tags) => tags.iter().any(|tag| tag.untagged()),
        Err(err) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if !untagged {
        let content = "All backgrounds have been tagged, \
            here are some random ones you can review again though";

        let builder = MessageBuilder::new().content(content);
        let _ = msg.create_message(&ctx, builder).await;
    }

    let mut owner = msg.author.id;

    loop {
        // Get all mapsets for which tags are missing
        let tags_result = if untagged {
            ctx.psql().get_all_tags_mapset(mode).await
        } else {
            ctx.psql()
                .get_random_tags_mapset(mode)
                .await
                .map(|tags| vec![tags])
        };

        let mapsets = match tags_result {
            Ok(mut tags) => {
                if untagged {
                    if tags.iter().any(|tag| tag.untagged()) {
                        tags.retain(|tag| tag.untagged());
                    } else {
                        let content = "All backgrounds have been tagged, \
                            here are some random ones you can review again though";

                        let builder = MessageBuilder::new().content(content);
                        let _ = msg.create_message(&ctx, builder).await;
                        untagged = false;
                        tags.truncate(1);
                    }
                }

                tags
            }
            Err(err) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        let (mapset_id, img) = get_random_image(mapsets, mode).await;

        let content = format!(
            "<@{owner}> Which tags should this mapsets get: {OSU_BASE}beatmapsets/{mapset_id}\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Lock in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Exit loop\n\
            ```"
        );

        let img = Attachment::from_bytes("bg_img.png".to_owned(), img);

        // Send response
        let response = ctx
            .http
            .create_message(msg.channel_id)
            .content(&content)?
            .attachments(&[img])
            .unwrap()
            .exec()
            .await?
            .model()
            .await?;

        let msg_id = response.id;

        // Setup collector
        let reaction_stream = ctx
            .standby
            .wait_for_event_stream(move |event: &Event| match event {
                Event::ReactionAdd(event) => {
                    event.message_id == msg_id && event.user_id.get() == OWNER_USER_ID
                }
                Event::ReactionRemove(event) => {
                    event.message_id == msg_id && event.user_id.get() == OWNER_USER_ID
                }
                _ => false,
            })
            .map(|event| match event {
                Event::ReactionAdd(add) => ReactionWrapper::Add(add.0),
                Event::ReactionRemove(remove) => ReactionWrapper::Remove(remove.0),
                _ => unreachable!(),
            })
            .timeout(Duration::from_secs(600));

        tokio::pin!(reaction_stream);

        // Add reactions
        let reactions = [
            "🍋",
            "🤓",
            "🤡",
            "🎨",
            "🍨",
            "👨‍🌾",
            "😱",
            "🪀",
            "🟦",
            "🗽",
            "🌀",
            "👴",
            "💯",
            "✅",
            "❌",
        ];

        for &reaction in reactions.iter() {
            let emote = Emote::Custom(reaction);
            send_reaction(&*ctx, &response, emote).await?;
        }

        let mut break_loop = true;

        // Run collector
        let mut add_tags = MapsetTags::empty();
        let mut remove_tags = MapsetTags::empty();

        while let Some(Ok(reaction)) = reaction_stream.next().await {
            let tag = if let ReactionType::Unicode { ref name } = reaction.as_deref().emoji {
                match name.as_str() {
                    "🍋" => MapsetTags::Easy,
                    "🤓" => MapsetTags::Hard,
                    "🤡" => MapsetTags::Meme,
                    "👴" => MapsetTags::Old,
                    "😱" => MapsetTags::HardName,
                    "🟦" => MapsetTags::BlueSky,
                    "🪀" => MapsetTags::Alternate,
                    "🗽" => MapsetTags::English,
                    "👨‍🌾" => MapsetTags::Farm,
                    "💯" => MapsetTags::Tech,
                    "🎨" => MapsetTags::Weeb,
                    "🌀" => MapsetTags::Streams,
                    "🍨" => MapsetTags::Kpop,
                    "✅" => {
                        owner = reaction.as_deref().user_id;
                        break_loop = false;

                        break;
                    }
                    "❌" => break,
                    _ => continue,
                }
            } else {
                continue;
            };

            match reaction {
                ReactionWrapper::Add(_) => {
                    add_tags.insert(tag);
                }
                ReactionWrapper::Remove(_) => {
                    remove_tags.insert(tag);
                }
            }
        }

        if !add_tags.is_empty() {
            if let Err(err) = ctx.psql().add_tags_mapset(mapset_id, add_tags).await {
                let report = Report::new(err).wrap_err("failed to add tags");
                warn!("{:?}", report);
            }
        }

        if !remove_tags.is_empty() {
            if let Err(err) = ctx.psql().remove_tags_mapset(mapset_id, remove_tags).await {
                let report = Report::new(err).wrap_err("failed to remove tags");
                warn!("{:?}", report);
            }
        }

        // Then show the final tags
        match ctx.psql().get_tags_mapset(mapset_id).await {
            Ok(tags) => {
                if !tags.is_empty() {
                    let content = format!(
                        "{}beatmapsets/{} is now tagged as:\n{}",
                        OSU_BASE, mapset_id, tags,
                    );

                    let builder = MessageBuilder::new().content(content);
                    msg.create_message(&ctx, builder).await?;
                }
            }
            Err(err) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        };

        if break_loop {
            let builder = MessageBuilder::new().content("Exiting loop :wave:");
            msg.create_message(&ctx, builder).await?;

            break;
        }
    }

    Ok(())
}

async fn get_random_image(mut mapsets: Vec<MapsetTagWrapper>, mode: GameMode) -> (u32, Vec<u8>) {
    let mut path = CONFIG.get().unwrap().paths.backgrounds.clone();

    match mode {
        GameMode::STD => path.push(OSU),
        GameMode::MNA => path.push(MANIA),
        _ => unreachable!(),
    }

    loop {
        let random_idx = {
            let mut rng = rand::thread_rng();
            rng.next_u32() as usize % mapsets.len()
        };

        let mapset = mapsets.swap_remove(random_idx);
        path.push(&mapset.filename);

        match fs::read(&path).await {
            Ok(bytes) => return (mapset.mapset_id, bytes),
            Err(err) => {
                let wrap = format!("error while reading file {}", path.display());
                let report = Report::new(err).wrap_err(wrap);
                warn!("{:?}", report);
                path.pop();
            }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Action {
    Add,
    Remove,
}

impl FromStr for Action {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.cow_to_ascii_lowercase().as_ref() {
            "r" | "remove" => Ok(Self::Remove),
            "a" | "add" => Ok(Self::Add),
            _ => Err(()),
        }
    }
}
