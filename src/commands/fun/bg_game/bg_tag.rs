use crate::{
    commands::checks::*,
    util::{
        globals::{HOMEPAGE, OSU_API_ISSUE},
        MessageExt,
    },
    MySQL, Osu,
};

use rosu::backend::BeatmapRequest;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::str::FromStr;

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
#[aliases("bgt", "bgtag")]
async fn bgtags(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.is_empty() {
        msg.channel_id
            .say(ctx, "Arguments: [mapset id] [add/a/remove/r] [list of tags]\n\
            Example: 21662 r hard farm streams alternate hardname tech weeb bluesky\n\
            Tags: farm, streams, alternate, old, meme, hardname, easy, hard, tech, weeb, bluesky, english")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    }
    // Parse mapset id
    // TODO: Check if mapset of available bg
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
    // If mapset not in general mapset table, request and add it
    if mysql.get_beatmapset(mapset_id).is_err() {
        let osu = data.get::<Osu>().unwrap();
        let req = BeatmapRequest::new().mapset_id(mapset_id);
        match req.queue(osu).await {
            Ok(maps) => {
                if let Err(why) = mysql.insert_beatmaps(maps) {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    error!("Error while adding missing mapset");
                    return Err(why.to_string().into());
                }
            }
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        }
    }
    // First add mapset to tag table, then update columns individually
    let mut result = mysql.add_tag_mapset(mapset_id);
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
            _ => return Err(()),
        }
    }
}

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
