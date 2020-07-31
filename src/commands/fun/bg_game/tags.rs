use super::ReactionWrapper;
use crate::{
    bail,
    bg_game::MapsetTags,
    core::CONFIG,
    database::MapsetTagWrapper,
    util::{
        constants::{GENERAL_ISSUE, OSU_BASE},
        MessageExt,
    },
    Args, BotResult, Context,
};

use rand::RngCore;
use rayon::prelude::*;
use rosu::models::GameMode;
use std::{str::FromStr, sync::Arc, time::Duration};
use tokio::{fs, stream::StreamExt};
use twilight::model::{
    channel::{Message, ReactionType},
    gateway::{
        event::{Event, EventType},
        payload::ReactionAdd,
    },
};

#[command]
#[short_desc("Help tagging backgrounds by tagging them manually")]
#[long_desc(
    "Manage the tags of a background for the bg game.\n\
    First argument must be the mapset id, second argument must be either \
    `a` or `add` to add tags, or `r` or `remove` to remove them. \n\
    After that provide any of these pre-selected keywords:\n\
    `farm, streams, alternate, old, meme, hardname, easy, hard, tech, weeb, bluesky, english`\n\
    By default, all tags are marked as **true**, so removing them will be more important.\n\
    **You need to be verified to use this command**, feel free to let \
    Badewanne3 know if you want to help out tagging backgrounds."
)]
#[usage("[mapset id] [add/a/remove/r] [list of tags]")]
#[example("21662 r hard farm streams alternate hardname tech weeb bluesky")]
#[aliases("bgtm", "bgtagmanual")]
async fn bgtagsmanual(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let verified_users_init = match ctx.psql().get_bg_verified().await {
        Ok(users) => users,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("Error while retrieving verified users: {}", why);
        }
    };
    if !verified_users_init.contains(&msg.author.id) {
        let content = "This command is only for verified people.\n\
            If you're interested in helping out tagging backgrounds, \
            feel free to let Badewanne3 know :)";
        return msg.error(&ctx, content).await;
    }
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
            return msg.respond(&ctx, content).await;
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
                    "Could not parse tag `{}`.\n\
                    Be sure to only give these tags:\n\
                    `farm, streams, alternate, old, meme, hardname, \
                    easy, hard, tech, weeb, bluesky, english`",
                    tag
                );
                return msg.error(&ctx, content).await;
            }
            None => unreachable!(),
        }
    }
    let result = if tags.is_empty() {
        ctx.psql().get_tags_mapset(mapset_id).await
    } else {
        let add = action == Action::Add;
        match ctx.psql().set_tags_mapset(mapset_id, tags, add).await {
            Ok(_) => ctx.psql().get_tags_mapset(mapset_id).await,
            Err(why) => Err(why),
        }
    };
    // Then show the final tags
    match result {
        Ok(tags) => {
            let content = format!(
                "{}beatmapsets/{} is now tagged as:\n{}",
                OSU_BASE, mapset_id, tags,
            );
            msg.respond(&ctx, content).await?;
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("Error while updating bg mapset tag: {}", why);
        }
    }
    Ok(())
}

#[command]
#[short_desc("Help out tagging backgrounds")]
#[long_desc(
    "Let me give you mapsets that still need to be tagged.\n\
    React to them properly, then lock it in by reacting with ✅.\n\
    To leave the loop, react with ❌ or just wait 10 minutes.\n\
    Mode can be specified in the first argument, defaults to std.\n\
    **You need to be verified to use this command**, feel free to \
    let Badewanne3 know if you want to help out tagging backgrounds."
)]
#[usage("[std / mna]")]
#[aliases("bgt", "bgtag")]
async fn bgtags(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let verified_users_init = match ctx.psql().get_bg_verified().await {
        Ok(users) => users,
        Err(why) => {
            let _ = msg.respond(&ctx, GENERAL_ISSUE).await;
            bail!("Error while retrieving verified users: {}", why);
        }
    };
    if !verified_users_init.contains(&msg.author.id) {
        let content = "This command is only for verified people.\n\
            If you're interested in helping out tagging backgrounds, \
            feel free to let Badewanne3 know :)";
        return msg.respond(&ctx, content).await;
    }
    // Parse arguments as mode
    let mode = match args.next() {
        Some(arg) => match arg.to_lowercase().as_str() {
            "mna" | "mania" | "m" => GameMode::MNA,
            "osu" | "std" | "standard" | "o" => GameMode::STD,
            _ => {
                let content = "Could not parse first argument as mode. \
                Provide either `mna`, or `std`";
                return msg.respond(&ctx, content).await;
            }
        },
        None => GameMode::STD,
    };
    let untagged = match ctx.psql().get_all_tags_mapset(mode).await {
        Ok(tags) => tags.par_iter().any(|tag| tag.untagged()),
        Err(why) => {
            let _ = msg.respond(&ctx, GENERAL_ISSUE).await;
            bail!("Error while getting all tags: {}", why);
        }
    };
    if !untagged {
        let content = "All backgrounds have been tagged, \
            here are some random ones you can review again though";
        let _ = msg.respond(&ctx, content).await;
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
            Ok(tags) => {
                if untagged {
                    tags.into_par_iter().filter(|tag| tag.untagged()).collect()
                } else {
                    tags
                }
            }
            Err(why) => {
                let _ = msg.respond(&ctx, GENERAL_ISSUE).await;
                bail!("Error while getting all / random tags: {}", why);
            }
        };
        let (mapset_id, img) = get_random_image(mapsets, mode).await;
        let content = format!(
            "<@{}> Which tags should this mapsets get: {}beatmapsets/{}\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Log in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Exit loop\n\
            ```",
            owner, OSU_BASE, mapset_id
        );

        // Send response
        let response = ctx
            .http
            .create_message(msg.channel_id)
            .content(content)?
            .attachment("bg_img.png", img)
            .await?;
        let msg_id = response.id;

        // Setup collector
        let verified_users = verified_users_init.clone();
        let reaction_remove_stream = ctx
            .standby
            .wait_for_event_stream(EventType::ReactionRemove, |_: &Event| true)
            .filter_map(move |event: Event| {
                if let Event::ReactionRemove(reaction) = event {
                    if reaction.0.message_id == msg_id
                        && verified_users.contains(&reaction.0.user_id)
                    {
                        return Some(ReactionWrapper::Remove(reaction.0));
                    }
                }
                None
            });
        let verified_users = verified_users_init.clone();
        let reaction_add_stream = ctx
            .standby
            .wait_for_reaction_stream(msg_id, move |event: &ReactionAdd| {
                verified_users.contains(&event.0.user_id)
            })
            .filter_map(|reaction: ReactionAdd| Some(ReactionWrapper::Add(reaction.0)));
        let mut reaction_stream = reaction_add_stream
            .merge(reaction_remove_stream)
            .timeout(Duration::from_secs(600));

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
            let emote = ReactionType::Unicode {
                name: reaction.to_string(),
            };
            ctx.http
                .create_reaction(response.channel_id, response.id, emote)
                .await?;
        }
        let mut break_loop = true;

        // Run collector
        let mut tags = MapsetTags::empty();
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
                    tags.insert(tag);
                }
                ReactionWrapper::Remove(_) => {
                    tags.remove(tag);
                }
            }
        }
        let result = if tags.is_empty() {
            ctx.psql().get_tags_mapset(mapset_id).await
        } else {
            match ctx.psql().set_tags_mapset(mapset_id, tags, true).await {
                Ok(_) => ctx.psql().get_tags_mapset(mapset_id).await,
                Err(why) => Err(why),
            }
        };

        // Then show the final tags
        match result {
            Ok(tags) => {
                if !tags.is_empty() {
                    let content = format!(
                        "{}beatmapsets/{} is now tagged as:\n{}",
                        OSU_BASE, mapset_id, tags,
                    );
                    msg.respond(&ctx, content).await?;
                }
            }
            Err(why) => {
                let _ = msg.respond(&ctx, GENERAL_ISSUE).await;
                bail!("Error while updating bg mapset tag: {}", why);
            }
        };
        if break_loop {
            let content = "Exiting loop, thanks for helping out :)";
            msg.respond(&ctx, content).await?;
            break;
        }
    }
    Ok(())
}

async fn get_random_image(mut mapsets: Vec<MapsetTagWrapper>, mode: GameMode) -> (u32, Vec<u8>) {
    let mut path = CONFIG.get().unwrap().bg_path.clone();
    match mode {
        GameMode::STD => path.push("osu"),
        GameMode::MNA => path.push("mania"),
        _ => unreachable!(),
    }
    loop {
        let random_idx = {
            let mut rng = rand::thread_rng();
            rng.next_u32() as usize % mapsets.len()
        };
        let mapset = mapsets.remove(random_idx);
        let filename = format!("{}.{}", mapset.mapset_id, mapset.filetype);
        path.push(filename);
        match fs::read(&path).await {
            Ok(bytes) => return (mapset.mapset_id, bytes),
            Err(why) => {
                warn!("Error while reading file {}: {}", path.display(), why);
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
        match value.to_lowercase().as_str() {
            "r" | "remove" => Ok(Self::Remove),
            "a" | "add" => Ok(Self::Add),
            _ => Err(()),
        }
    }
}
