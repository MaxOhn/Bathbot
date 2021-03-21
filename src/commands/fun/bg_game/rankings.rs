use crate::{
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, get_member_ids, numbers, MessageExt},
    Args, BotResult, Context,
};

use cow_utils::CowUtils;
use hashbrown::HashMap;
use std::sync::Arc;
use twilight_model::{channel::Message, id::UserId};

#[command]
#[short_desc("Show the user rankings for the game")]
#[aliases("rankings", "leaderboard", "lb", "stats")]
pub async fn rankings(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let server_opt = args
        .next()
        .filter(|_| msg.guild_id.is_some())
        .map(CowUtils::cow_to_lowercase)
        .filter(|arg| arg.as_ref() == "server" || arg.as_ref() == "s");

    if server_opt.is_some() {
        let guild_id = msg.guild_id.unwrap();

        let member_count = ctx
            .cache
            .guild(guild_id)
            .and_then(|guild| guild.member_count)
            .unwrap_or(0);

        let wait_msg = if member_count > 6000 {
            msg.respond(&ctx, "Lots of members, give me a moment...")
                .await
                .ok()
        } else {
            None
        };

        let members = match get_member_ids(&ctx, guild_id).await {
            Ok(members) => members,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };

        if let Some(msg) = wait_msg {
            let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
        }

        scores.retain(|(id, _)| members.contains(id));
    }

    scores.sort_unstable_by(|(_, a), (_, b)| b.cmp(&a));
    let author_idx = scores.iter().position(|(user, _)| *user == msg.author.id.0);

    // Gather usernames for initial page
    let mut usernames = HashMap::with_capacity(15);

    for &id in scores.iter().take(15).map(|(id, _)| id) {
        let name = match ctx.cache.user(UserId(id)) {
            Some(user) => user.name.to_owned(),
            None => match ctx.http.user(UserId(id)).await {
                Ok(Some(user)) => user.name,
                Ok(None) | Err(_) => String::from("Unknown user"),
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
    let data = BGRankingEmbed::new(author_idx, initial_scores, 1, (1, pages));

    // Creating the embed
    let embed = data.build().build()?;

    let response = msg.respond_embed(&ctx, embed).await?;

    // Skip pagination if too few entries
    if scores.len() <= 15 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination =
        BGRankingPagination::new(Arc::clone(&ctx), response, author_idx, scores, usernames);

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (bgranking): {}")
        }
    });

    Ok(())
}
