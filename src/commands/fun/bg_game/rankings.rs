use crate::{
    bail,
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, numbers, MessageExt},
    Args, BotResult, Context,
};

use std::{collections::HashMap, sync::Arc};
use twilight_model::{channel::Message, id::UserId};

#[command]
#[short_desc("Show the user rankings for the game")]
#[aliases("rankings", "leaderboard", "lb", "stats")]
pub async fn rankings(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(why) => {
            msg.error(&ctx, GENERAL_ISSUE).await?;
            bail!("error while getting all bggame scores: {}", why);
        }
    };
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
        .map(|(id, score)| (usernames.get(&id).unwrap(), *score))
        .collect();

    // Prepare initial page
    let pages = numbers::div_euclid(15, scores.len());
    let data = BGRankingEmbed::new(author_idx, initial_scores, 1, (1, pages));

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

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
            warn!("Pagination error (bgranking): {}", why)
        }
    });
    Ok(())
}
