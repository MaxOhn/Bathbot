#![allow(non_upper_case_globals)]

use crate::{
    commands::checks::*,
    database::MapsetTagWrapper,
    util::{globals::HOMEPAGE, MessageExt},
    BgVerified, MySQL,
};

use rand::RngCore;
use rosu::models::GameMode;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::{Message, ReactionType},
    prelude::Context,
};
use std::{
    convert::TryFrom, env, fmt::Write, hash::Hash, path::PathBuf, str::FromStr, time::Duration,
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
    let mut tags = MapsetTags::empty();
    while !args.is_empty() {
        match args.single::<MapsetTags>() {
            Ok(tag) => tags.insert(tag),
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
    let result = if tags.is_empty() {
        Ok(())
    } else {
        mysql.set_tags_mapset(mapset_id, tags, action == Action::Add)
    };
    // Then show the final tags
    let response = match result.and_then(|_| mysql.get_tags_mapset(mapset_id)) {
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
#[description = "Let me give you mapsets that still need to be tagged.\n\
React to them properly, then lock it in by reacting with ✅.\n\
To leave the loop, react with ❌ or just wait 10 minutes.\n\
Mode can be specified in the first argument, defaults to std."]
#[usage = "[std / mna]"]
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
    let untagged = {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.get_all_tags_mapset(mode) {
            Ok(tags) => tags.iter().filter(|tag| tag.untagged()).count() > 0,
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
    if !untagged {
        msg.channel_id
            .say(
                ctx,
                "All backgrounds have been tagged, \
                here are some random ones you can review again though",
            )
            .await?;
    }
    loop {
        // Get all mapsets for which tags are missing
        let mapsets = {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            let tags_result = if untagged {
                mysql.get_all_tags_mapset(mode)
            } else {
                mysql.get_random_tags_mapset(mode).map(|tags| vec![tags])
            };
            match tags_result {
                Ok(tags) => {
                    if untagged {
                        tags.into_iter()
                            .filter(|tag| tag.untagged())
                            .collect::<Vec<_>>()
                    } else {
                        tags
                    }
                }
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
        let (mapset_id, img) = get_random_image(mapsets, mode).await;
        let content = format!(
            "Which tags should this mapsets get: {}beatmapsets/{}\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Log in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Exit loop\n\
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
        let verified_users = {
            let data = ctx.data.read().await;
            data.get::<BgVerified>().unwrap().clone()
        };
        let mut collector = response
            .await_reactions(ctx)
            .timeout(Duration::from_secs(600))
            .filter(move |reaction| verified_users.contains(&reaction.user_id))
            .removed(true)
            .await;
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
            let reaction = ReactionType::try_from(reaction).unwrap();
            response.react(ctx, reaction).await?;
        }
        let mut break_loop = true;
        // Run collector
        let mut tags = MapsetTags::empty();
        while let Some(reaction) = collector.next().await {
            let tag = if let ReactionType::Unicode(ref reaction) = reaction.as_inner_ref().emoji {
                match reaction.as_str() {
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
                        break_loop = false;
                        break;
                    }
                    "❌" => break,
                    _ => continue,
                }
            } else {
                continue;
            };
            if reaction.is_added() {
                tags.insert(tag);
            } else {
                tags.remove(tag);
            }
        }
        collector.stop();
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        let result = if tags.is_empty() {
            Ok(())
        } else {
            mysql.set_tags_mapset(mapset_id, tags, true)
        };
        // Then show the final tags
        match result.and_then(|_| mysql.get_tags_mapset(mapset_id)) {
            Ok(tags) => {
                let content = format!(
                    "{}beatmapsets/{} is now tagged as:\n{}",
                    HOMEPAGE, mapset_id, tags,
                );
                msg.channel_id.say(ctx, content).await?;
            }
            Err(why) => {
                error!("Error while updating bg mapset tag: {}", why);
                msg.channel_id
                    .say(ctx, "Some database issue, blame bade")
                    .await?;
            }
        };
        if break_loop {
            msg.channel_id
                .say(ctx, "Loop quitted, thanks for helping out :)")
                .await?;
            break;
        }
    }
    Ok(())
}

async fn get_random_image(mut mapsets: Vec<MapsetTagWrapper>, mode: GameMode) -> (u32, Vec<u8>) {
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

bitflags! {
    pub struct MapsetTags: u32 {
        const Farm = 1;
        const Streams = 2;
        const Alternate = 4;
        const Old = 8;
        const Meme = 16;
        const HardName = 32;
        const Easy = 64;
        const Hard = 128;
        const Tech = 256;
        const Weeb = 512;
        const BlueSky = 1024;
        const English = 2048;
        const Kpop = 4096;
    }
}

impl Default for MapsetTags {
    fn default() -> Self {
        Self::all()
    }
}

impl FromStr for MapsetTags {
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

impl MapsetTags {
    pub fn join(self, separator: &str) -> String {
        let mut tags = self.into_iter();
        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => return "None".to_owned(),
        };
        let mut result = String::with_capacity(16);
        let _ = write!(result, "{:?}", first_tag);
        for element in tags {
            let _ = write!(result, "{}{:?}", separator, element);
        }
        result
    }
}

pub struct IntoIter {
    tags: MapsetTags,
    shift: usize,
}

impl Iterator for IntoIter {
    type Item = MapsetTags;
    fn next(&mut self) -> Option<Self::Item> {
        if self.tags.is_empty() {
            None
        } else {
            loop {
                if self.shift == 32 {
                    return None;
                }
                let bit = 1 << self.shift;
                self.shift += 1;
                if self.tags.bits & bit != 0 {
                    return MapsetTags::from_bits(bit);
                }
            }
        }
    }
}

impl IntoIterator for MapsetTags {
    type Item = MapsetTags;
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        IntoIter {
            tags: self,
            shift: 0,
        }
    }
}
