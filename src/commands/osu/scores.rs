use crate::{
    messages::{BotEmbed, EmbedType},
    util::globals::OSU_API_ISSUE,
    Osu,
};

use rosu::{
    backend::requests::{BeatmapArgs, OsuArgs, OsuRequest, ScoreArgs, UserArgs},
    models::{Beatmap, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

#[command]
#[description = "Display scores for all mods that a user has on a map. \
Beatmap can be given as url or just **mapid**. \
If no beatmap is given, it will choose the map of a score in the channel's history"]
#[example = "2240404 badewanne3"]
#[aliases("c", "compare")]
fn scores(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let map_id: u32 = args.single()?;
    let name: String = args.single_quoted()?;
    let score_args = ScoreArgs::with_map_id(map_id).username(&name);
    let user_args = UserArgs::with_username(&name);
    let map_args = BeatmapArgs::new().map_id(map_id);
    let (score_req, user_req, map_req): (OsuRequest<Score>, OsuRequest<User>, OsuRequest<Beatmap>) = {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get osu client");
        let score_req = osu.create_request(OsuArgs::Scores(score_args));
        let user_req = osu.create_request(OsuArgs::Users(user_args));
        let map_req = osu.create_request(OsuArgs::Beatmaps(map_args));
        (score_req, user_req, map_req)
    };
    let mut rt = Runtime::new().unwrap();

    // Retrieve map, user, and user's scores on the map
    let res = rt.block_on(async {
        let scores = match score_req.queue().await {
            Ok(scores) => scores,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving Scores: {}",
                    why
                )));
            }
        };
        let users = match user_req.queue().await {
            Ok(users) => users,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving Users: {}",
                    why
                )));
            }
        };
        let maps = match map_req.queue().await {
            Ok(maps) => maps,
            Err(why) => {
                return Err(CommandError(format!(
                    "Error while retrieving Beatmaps: {}",
                    why
                )));
            }
        };
        Ok((scores, users, maps))
    });
    let (scores, user, map) = match res {
        Ok((scores, mut users, mut maps)) => {
            let user = match users.pop() {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User {} was not found", name))?;
                    return Ok(());
                }
            };
            let map = match maps.pop() {
                Some(map) => map,
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        format!("Beatmap with id {} was not found", map_id),
                    )?;
                    return Ok(());
                }
            };
            (scores, user, map)
        }
        Err(why) => {
            msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
            return Err(why);
        }
    };

    // Creating the embed
    let embed = BotEmbed::new(
        ctx.cache.clone(),
        map.mode,
        EmbedType::UserScoreMulti(Box::new(user), Box::new(map), scores),
    );
    let _ = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| embed.create(e)));
    Ok(())
}
