use crate::{
    arguments::OsuStatsArgs,
    embeds::{EmbedData, RankEmbed},
    scraper::Scraper,
    util::{globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu,
};

use rosu::{
    backend::requests::UserRequest,
    models::{GameMode, Score, User},
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

async fn rank_send(mode: GameMode, ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = match RankArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.channel_id
                .say(ctx, err_msg)
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
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
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
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
                    msg.channel_id
                        .say(ctx, format!("User id `{}` was not found", rank_holder_id))
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
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
                    msg.channel_id
                        .say(ctx, format!("User `{}` was not found", name))
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id
                    .say(ctx, OSU_API_ISSUE)
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Err(why.to_string().into());
            }
        };
        if user.pp_raw > rank_holder.pp_raw {
            (user, Vec::with_capacity(0))
        } else {
            let scores = match user.get_top_scores(&osu, 100, mode).await {
                Ok(scores) => scores,
                Err(why) => {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(why.to_string().into());
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
#[description = "Calculate how many more pp a player requires to \
                 reach a given rank"]
#[usage = "[username] [[country]number]"]
#[example = "badewanne3 be50"]
#[example = "badewanne3 123"]
#[aliases("reach")]
pub async fn placeholder(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    rank_send(GameMode::STD, ctx, msg, args).await
}

// #[command]
// #[description = "Calculate how many more pp a mania player requires to \
//                  reach a given rank"]
// #[example = "badewanne3 be50"]
// #[example = "badewanne3 123"]
// #[aliases("rankm", "reachmania", "reachm")]
// pub async fn rankmania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     rank_send(GameMode::MNA, ctx, msg, args).await
// }

// #[command]
// #[description = "Calculate how many more pp a taiko player requires to \
//                  reach a given rank"]
// #[example = "badewanne3 be50"]
// #[example = "badewanne3 123"]
// #[aliases("rankt", "reachtaiko", "reacht")]
// pub async fn ranktaiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     rank_send(GameMode::TKO, ctx, msg, args).await
// }

// #[command]
// #[description = "Calculate how many more pp a ctb player requires to \
//                  reach a given rank"]
// #[example = "badewanne3 be50"]
// #[example = "badewanne3 123"]
// #[aliases("rankc", "reachctb", "reachc")]
// pub async fn rankctb(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     rank_send(GameMode::CTB, ctx, msg, args).await
// }
