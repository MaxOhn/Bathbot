use crate::{
    core::MatchTrackResult,
    util::{
        constants::{OSU_API_ISSUE, OSU_BASE},
        matcher, MessageExt,
    },
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Live track a multiplayer match")]
#[long_desc(
    "Live track a multiplayer match in a channel.\n\
    Similar to what an mp link does, I will keep a channel up \
    to date about events in a match.\n\
    Use the `matchliveremove` command to stop tracking the match."
)]
#[usage("[match url / match id]")]
#[example("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("ml", "mla", "matchliveadd", "mlt", "matchlivetrack")]
#[bucket("match_live")]
async fn matchlive(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let match_id = match args.next().and_then(matcher::get_osu_match_id) {
        Some(arg) => arg,
        None => {
            let content = "The first argument must be either a match \
            id or the multiplayer link to a match";

            return msg.error(&ctx, content).await;
        }
    };

    let content: Result<&str, _> = match ctx.add_match_track(msg.channel_id, match_id).await {
        MatchTrackResult::Added => return Ok(()),
        MatchTrackResult::Capped => Err("Channels can track at most three games at a time"),
        MatchTrackResult::Duplicate => Err("That match is already being tracking in this channel"),
        MatchTrackResult::Error => Err(OSU_API_ISSUE),
    };

    match content {
        Ok(content) => msg.send_response(&ctx, content).await,
        Err(content) => msg.error(&ctx, content).await,
    }
}

#[command]
#[short_desc("Untrack a multiplayer match")]
#[long_desc(
    "Untrack a multiplayer match in a channel.\n\
    The match id only has to be specified in case the channel \
    currently live tracks more than one match."
)]
#[usage("[match url / match id]")]
#[example("58320988", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mlr")]
async fn matchliveremove(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let match_id_opt = args
        .next()
        .and_then(matcher::get_osu_match_id)
        .or_else(|| ctx.tracks_single_match(msg.channel_id));

    let match_id = match match_id_opt {
        Some(match_id) => match_id,
        None => {
            let content = "The channel does not track exactly one match \
            and the match id could not be parsed from the first argument.\n\
            Try specifying the match id as first argument.";

            return msg.error(&ctx, content).await;
        }
    };

    if ctx.remove_match_track(msg.channel_id, match_id) {
        let content = format!(
            "Stopped live tracking [the match]({}community/matches/{})",
            OSU_BASE, match_id
        );

        msg.send_response(&ctx, content).await
    } else {
        let content = "The match wasn't tracked in this channel";

        msg.error(&ctx, content).await
    }
}
