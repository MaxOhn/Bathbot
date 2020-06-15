use crate::{
    commands::checks::*,
    database::MapsetTagDB,
    util::{globals::HOMEPAGE, MessageExt},
    MySQL,
};

use rand::RngCore;
use rosu::models::GameMode;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{
    collections::HashSet, convert::TryFrom, env, hash::Hash, path::PathBuf, str::FromStr,
    time::Duration,
};
use tokio::{fs, stream::StreamExt};

#[command]
#[checks(BgVerified)]
#[description = "Manage the tags of a background for the bg game.\n\
First argument must be the mapset id, second argument must be either \
`a` or `add` to add tags, or `r` or `remove` to remove them. \n\
After that provide any of these pre-selected keywords:\n\
`farm, streams, alternate, old, meme, hardname, easy, hard, tech, weeb, bluesky, english`\n\
By default, all tags are marked as **true**, so removing them will be more important."]
#[usage = "[mapset id] [add/a/remove/r] [list of tags]"]
#[example = "21662 r hard farm streams alternate hardname tech weeb bluesky"]
#[aliases("bgtm", "bgtagmanual")]
async fn bgtagsmanual(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.is_empty() {
        msg.channel_id
            .say(ctx, "Arguments: `[mapset id] [add/a/remove/r] [list of tags]`\n\
            Example: `21662 r hard farm streams alternate hardname tech weeb bluesky`\n\
            Tags: `farm, streams, alternate, old, meme, hardname, easy, hard, tech, weeb, bluesky, english`")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    }
    // Parse mapset id
    let mapset_id = match args.single::<u32>() {
        Ok(id) => id,
        Err(_) => {
            msg.channel_id
                .say(
                    ctx,
                    "Could not parse mapset id. Be sure to specify it as first argument",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
    };
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        if mysql.get_tags_mapset(mapset_id).is_err() {
            msg.channel_id
                .say(ctx, "No background entry found with this id")
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
    }
    // Parse action
    let action = match args.single::<Action>() {
        Ok(action) => action,
        Err(_) => {
            msg.channel_id
                .say(
                    ctx,
                    "Could not parse action. \
                    Be sure to specify `r`, `remove`, `a`, or `add` as second argument",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
    };
    // Parse tags
    let mut tags = Vec::new();
    while !args.is_empty() {
        match args.single::<MapsetTag>() {
            Ok(tag) => tags.push(tag),
            Err(tag) => {
                msg.channel_id
                    .say(
                        ctx,
                        format!(
                            "Could not parse tag `{}`.\n\
                            Be sure to only give these tags:\n\
                            `farm, streams, alternate, old, meme, hardname, \
                            easy, hard, tech, weeb, bluesky, english`",
                            tag
                        ),
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    }
    let data = ctx.data.read().await;
    let mysql = data.get::<MySQL>().unwrap();
    // Update columns individually
    let mut result = Ok(());
    for tag in tags {
        result = result.and(mysql.set_tag_mapset(mapset_id, tag, action == Action::Add));
    }
    // Then show the final tags
    let result = result.and_then(|_| mysql.get_tags_mapset(mapset_id));
    let response = match result {
        Ok(tags) => {
            msg.channel_id
                .say(
                    ctx,
                    format!(
                        "{}beatmapsets/{} is now tagged as:\n{}",
                        HOMEPAGE, mapset_id, tags,
                    ),
                )
                .await?
        }
        Err(why) => {
            error!("Error while updating bg mapset tag: {}", why);
            msg.channel_id
                .say(ctx, "Some database issue, blame bade")
                .await?
        }
    };
    response.reaction_delete(ctx, msg.author.id).await;
    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Let me give you mapsets that still need to be tagged.\n\
React to them properly, then finish it up by either waiting 10min or react with ✅.\n\
To leave the loop, react with ❌."]
#[aliases("bgt", "bgtag")]
async fn bgtags(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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
    loop {
        // Get all mapsets for which tags are missing
        let missing_tags = {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            match mysql.get_all_tags_mapset(mode) {
                Ok(tags) => tags
                    .into_iter()
                    .filter(|tag| tag.untagged())
                    .collect::<Vec<_>>(),
                Err(why) => {
                    msg.channel_id
                        .say(ctx, "Some database issue, blame bade")
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
                }
            }
        };
        if missing_tags.is_empty() {
            msg.channel_id
                .say(
                    ctx,
                    "All background entries have been tagged, no untagged one left",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
        let (mapset_id, img) = get_random_image(missing_tags, mode).await;
        let content = format!(
            "This mapset needs some tags {}beatmapsets/{}\n\
        ```\n\
        🍋: Easy  😱: Hard name  👨‍🌾: Farm\n\
        🤓: Hard  🏙️: Blue sky   💯: Tech\n\
        🤡: Meme  🪀: Alternate  🤢: Weeb\n\
        👴: Old   🆒: English    🚅: Streams\n\
        ```",
            HOMEPAGE, mapset_id
        );
        // Send response
        let response = msg
            .channel_id
            .send_message(ctx, |m| {
                m.content(content).add_file((img.as_slice(), "bg_img.png"))
            })
            .await?;
        // Setup collector
        let mut collector = response
            .await_reactions(ctx)
            .timeout(Duration::from_secs(600))
            .author_id(msg.author.id)
            .removed(true)
            .await;
        // Add reactions
        let reactions = [
            "🍋",
            "🤓",
            "🤡",
            "👴",
            "😱",
            "🏙️",
            "🪀",
            "🆒",
            "👨‍🌾",
            "💯",
            "🤢",
            "🚅",
            "✅",
            "❌",
        ];
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            response.react(ctx, reaction_type).await?;
        }
        let mut break_loop = true;
        // Run collector
        let mut tags = HashSet::new();
        while let Some(reaction) = collector.next().await {
            let tag = if let ReactionType::Unicode(ref reaction) = reaction.as_inner_ref().emoji {
                match reaction.as_str() {
                    "🍋" => MapsetTag::Easy,
                    "🤓" => MapsetTag::Hard,
                    "🤡" => MapsetTag::Meme,
                    "👴" => MapsetTag::Old,
                    "😱" => MapsetTag::HardName,
                    "🏙️" => MapsetTag::BlueSky,
                    "🪀" => MapsetTag::Alternate,
                    "🆒" => MapsetTag::English,
                    "👨‍🌾" => MapsetTag::Farm,
                    "💯" => MapsetTag::Tech,
                    "🤢" => MapsetTag::Weeb,
                    "🚅" => MapsetTag::Streams,
                    "✅" => {
                        break_loop = false;
                        break;
                    }
                    "❌" => {
                        msg.channel_id
                            .say(ctx, "Loop quited, thanks for helping out :)")
                            .await?;
                        return Ok(());
                    }
                    _ => continue,
                }
            } else {
                continue;
            };
            if reaction.is_added() {
                tags.insert(tag);
            } else {
                tags.remove(&tag);
            }
        }
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        // Update columns individually
        let mut result = Ok(());
        for tag in tags {
            result = result.and(mysql.set_tag_mapset(mapset_id, tag, true));
        }
        // Then show the final tags
        let result = result.and_then(|_| mysql.get_tags_mapset(mapset_id));
        match result {
            Ok(tags) => {
                msg.channel_id
                    .say(
                        ctx,
                        format!(
                            "{}beatmapsets/{} is now tagged as:\n{}",
                            HOMEPAGE, mapset_id, tags,
                        ),
                    )
                    .await?;
            }
            Err(why) => {
                error!("Error while updating bg mapset tag: {}", why);
                msg.channel_id
                    .say(ctx, "Some database issue, blame bade")
                    .await?;
            }
        };
        if break_loop {
            break;
        }
    }
    Ok(())
}

async fn get_random_image(mut missing_tags: Vec<MapsetTagDB>, mode: GameMode) -> (u32, Vec<u8>) {
    let mut path = PathBuf::new();
    path.push(env::var("BG_PATH").unwrap());
    match mode {
        GameMode::STD => path.push("osu"),
        GameMode::MNA => path.push("mania"),
        _ => unreachable!(),
    }
    loop {
        let random_idx = {
            let mut rng = rand::thread_rng();
            rng.next_u32() as usize % missing_tags.len()
        };
        let mapset = missing_tags.remove(random_idx);
        let filename = format!(
            "{}.{}",
            mapset.beatmapset_id,
            mapset.filetype.as_ref().unwrap()
        );
        path.push(filename);
        match fs::read(&path).await {
            Ok(bytes) => return (mapset.beatmapset_id, bytes),
            Err(why) => {
                warn!("Error while reading file {}: {}", path.display(), why);
                path.pop();
                continue;
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MapsetTag {
    Farm,
    Streams,
    Alternate,
    Old,
    Meme,
    HardName,
    Easy,
    Hard,
    Tech,
    BlueSky,
    English,
    Weeb,
}

impl FromStr for MapsetTag {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let result = match value.to_lowercase().as_str() {
            "farm" => Self::Farm,
            "stream" | "streams" => Self::Streams,
            "alt" | "alternate" => Self::Alternate,
            "old" | "oldschool" => Self::Old,
            "meme" => Self::Meme,
            "hardname" => Self::HardName,
            "easy" => Self::Easy,
            "hard" => Self::Hard,
            "tech" | "technical" => Self::Tech,
            "bluesky" => Self::BlueSky,
            "english" => Self::English,
            "weeb" | "anime" => Self::Weeb,
            other => return Err(other.to_owned()),
        };
        Ok(result)
    }
}
