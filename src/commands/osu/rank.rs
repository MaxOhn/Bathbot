use crate::{
    arguments::{Args, RankArgs},
    embeds::{EmbedData, RankEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use std::sync::Arc;
use twilight::model::channel::Message;

async fn rank_send(mode: GameMode, ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let args = match RankArgs::new(Args::new(msg.content.clone())) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };
    let country = args.country;
    let rank = args.rank;

    // Retrieve the rank holding user
    let rank_holder_id = {
        let data = ctx.data.read().await;
        let scraper = data.get::<Scraper>().unwrap();
        match scraper
            .get_userid_of_rank(rank, mode, country.as_deref())
            .await
        {
            Ok(rank) => rank,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };
    let rank_holder = {
        let user_req = UserRequest::with_user_id(rank_holder_id).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    let content = format!("User id `{}` was not found", rank_holder_id);
                    msg.respond(ctx, content).await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };

    // Retrieve the user (and its top scores if user has more pp than rank_holder)
    let (user, scores): (User, Vec<Score>) = {
        let user_req = UserRequest::with_username(&name).mode(mode);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let user = match user_req.queue_single(&osu).await {
            Ok(result) => match result {
                Some(user) => user,
                None => {
                    let content = format!("User `{}` was not found", name);
                    msg.respond(ctx, content).await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        if user.pp_raw > rank_holder.pp_raw {
            (user, Vec::with_capacity(0))
        } else {
            let scores = match user.get_top_scores(&osu, 100, mode).await {
                Ok(scores) => scores,
                Err(why) => {
                    msg.respond(&ctx, OSU_API_ISSUE).await?;
                    return Err(why.into());
                }
            };
            (user, scores)
        }
    };

    // Accumulate all necessary data
    let data = RankEmbed::new(user, scores, rank, country, rank_holder);

    // Creating the embed
    msg.channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[usage = "[username] [[country]number]"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("reach")]
pub async fn rank(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    rank_send(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankm", "reachmania", "reachm")]
pub async fn rankmania(ctx: &Context, msg: &Message, args: Args) -> BotResult<()> {
    rank_send(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankt", "reachtaiko", "reacht")]
pub async fn ranktaiko(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    rank_send(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[example("badewanne3 be50")]
#[example("badewanne3 123")]
#[aliases("rankc", "reachctb", "reachc")]
pub async fn rankctb(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    rank_send(GameMode::CTB, ctx, msg, args).await
}
