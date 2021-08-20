use crate::{
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, get_member_ids, numbers, CowUtils, MessageExt},
    BotResult, CommandData, Context,
};

use hashbrown::HashMap;
use std::sync::Arc;
use twilight_model::id::UserId;

#[command]
#[short_desc("Show the user rankings for the game")]
#[aliases("rankings", "leaderboard", "lb", "stats")]
async fn rankings(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let global = !args
                .next()
                .filter(|_| msg.guild_id.is_some())
                .map(CowUtils::cow_to_ascii_lowercase)
                .filter(|arg| arg == "server" || arg == "s")
                .is_some();

            _rankings(ctx, CommandData::Message { msg, args, num }, global).await
        }
        CommandData::Interaction { command } => super::slash_backgroundgame(ctx, command).await,
    }
}

pub(super) async fn _rankings(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    global: bool,
) -> BotResult<()> {
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let guild_id = data.guild_id();

    if !global {
        let guild_id = guild_id.unwrap();

        // let member_count = ctx
        //     .cache
        //     .guild(guild_id)
        //     .and_then(|guild| guild.member_count)
        //     .unwrap_or(0);

        // TODO
        // let wait_msg = if member_count > 6000 {
        //     msg.respond(&ctx, "Lots of members, give me a moment...")
        //         .await
        //         .ok()
        // } else {
        //     None
        // };

        let members = match get_member_ids(&ctx, guild_id).await {
            Ok(members) => members,
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };

        // if let Some(msg) = wait_msg {
        //     let _ = ctx.http.delete_message(msg.channel_id, msg.id).exec().await;
        // }

        scores.retain(|(id, _)| members.contains(id));
    }

    let author_id = data.author()?.id;

    scores.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
    let author_idx = scores.iter().position(|(user, _)| *user == author_id.0);

    // Gather usernames for initial page
    let mut usernames = HashMap::with_capacity(15);

    for &id in scores.iter().take(15).map(|(id, _)| id) {
        let name = match ctx.cache.user(UserId(id)) {
            Some(user) => user.name.to_owned(),
            None => match ctx.http.user(UserId(id)).exec().await {
                Ok(user_res) => match user_res.model().await {
                    Ok(user) => user.name,
                    Err(_) => String::from("Unknown user"),
                },
                Err(_) => String::from("Unknown user"),
            },
        };

        usernames.insert(id, name);
    }

    let initial_scores = scores
        .iter()
        .take(15)
        .map(|(id, score)| (&usernames[id], *score))
        .collect();

    // Prepare initial page
    let pages = numbers::div_euclid(15, scores.len());
    let embed_data = BGRankingEmbed::new(author_idx, initial_scores, 1, (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 15 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination =
        BGRankingPagination::new(Arc::clone(&ctx), response, author_idx, scores, usernames);

    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (bgranking): {}")
        }
    });

    Ok(())
}
