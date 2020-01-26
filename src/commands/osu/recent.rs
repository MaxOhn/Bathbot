use crate::{
    messages::{BotEmbed, EmbedType},
    util::globals::OSU_API_ISSUE,
    Osu,
};

use rosu::{
    backend::requests::{OsuArgs, OsuRequest, ScoreArgs, UserBestArgs, UserRecentArgs},
    models::{Beatmap, GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

fn recent_send(mode: GameMode, ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let name: String = args.single_quoted()?;
    let recent_args = UserRecentArgs::with_username(&name).mode(mode).limit(1);
    let recent_req: OsuRequest<Score> = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        osu.create_request(OsuArgs::Recent(recent_args))
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve the recent score
    let score: Score = match rt.block_on(recent_req.queue()) {
        Ok(mut scores) => {
            if let Some(score) = scores.pop() {
                score
            } else {
                msg.channel_id.say(
                    &ctx.http,
                    format!("No recent plays found for user `{}`", name),
                )?;
                return Ok(());
            }
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(CommandError(format!(
                "Error while retrieving UserRecent: {}",
                why
            )));
        }
    };

    // Retrieving the score's user and beatmap
    let res = rt.block_on(async {
        let user: User = match score.user.get(mode).await {
            Ok(u) => u,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving LazilyLoaded<User> of recent: {}",
                    why
                )));
            }
        };
        let map: Beatmap = match score.beatmap.as_ref().unwrap().get(mode).await {
            Ok(m) => m,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving LazilyLoaded<Beatmap> of recent: {}",
                    why
                )));
            }
        };
        Ok((user, map))
    });
    let (user, map) = match res {
        Ok(tuple) => tuple,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Retrieving the user's top 100 and the map's global top 50
    let best_args = UserBestArgs::with_username(&name).mode(mode).limit(100);
    let global_args = ScoreArgs::with_map_id(score.beatmap_id.unwrap())
        .mode(mode)
        .limit(50);
    let (best_req, global_req) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let best_req = osu.create_request(OsuArgs::Best(best_args));
        let global_req = osu.create_request(OsuArgs::Scores(global_args));
        (best_req, global_req)
    };
    let res = rt.block_on(async {
        let best: Vec<Score> = match best_req.queue().await {
            Ok(scores) => scores,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving UserBest: {}",
                    why
                )));
            }
        };
        let global: Vec<Score> = match global_req.queue().await {
            Ok(scores) => scores,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving Scores: {}",
                    why
                )));
            }
        };
        Ok((best, global))
    });
    let (best, global) = match res {
        Ok(tuple) => tuple,
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Creating the embed
    let embed = BotEmbed::new(
        ctx.cache.clone(),
        mode,
        EmbedType::UserScoreSingle(Box::new(user), Box::new(score), Box::new(map), best, global),
    );
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}

#[command]
#[description = "Display a user's most recent play"]
#[usage = "badewanne3"]
#[aliases("r", "rs")]
pub fn recent(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::STD, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent mania play"]
#[usage = "badewanne3"]
#[aliases("rm")]
pub fn recentmania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::MNA, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent taiko play"]
#[usage = "badewanne3"]
#[aliases("rt")]
pub fn recenttaiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::TKO, ctx, msg, args)
}

#[command]
#[description = "Display a user's most recent ctb play"]
#[usage = "badewanne3"]
#[aliases("rc")]
pub fn recentctb(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    recent_send(GameMode::CTB, ctx, msg, args)
}
