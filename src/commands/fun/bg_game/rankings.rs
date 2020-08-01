use crate::{
    bail,
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, numbers, MessageExt},
    Args, BotResult, Context,
};

use std::{collections::HashMap, sync::Arc};
use twilight::model::{channel::Message, id::UserId};

#[command]
#[short_desc("Show the user rankings for the game")]
#[aliases("rankings", "leaderboard", "lb", "stats")]
pub async fn rankings(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let global = match args.next() {
        Some("g") | Some("global") => true,
        _ => msg.guild_id.is_none(),
    };
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(why) => {
            msg.error(&ctx, GENERAL_ISSUE).await?;
            bail!("error while getting all bggame scores: {}", why);
        }
    };

    // Filter only guild members if not global and in a guild
    if !global && msg.guild_id.is_some() {
        let guild_id = msg.guild_id.unwrap();
        let member_ids: Vec<_> = match ctx.cache.get_guild(guild_id) {
            Some(guild) => guild.members.iter().map(|guard| guard.key().0).collect(),
            None => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("guild {} not in cache", guild_id);
            }
        };
        scores.retain(|(user, _)| member_ids.iter().any(|member| member == user));
        if scores.is_empty() {
            let content = "Looks like no one on this server has played the background game yet";
            return msg.respond(&ctx, content).await;
        }
    }
    scores.sort_by(|(_, a), (_, b)| b.cmp(&a));
    let author_idx = scores.iter().position(|(user, _)| *user == msg.author.id.0);

    // Gather usernames for initial page
    let mut usernames = HashMap::with_capacity(15);
    for &id in scores.iter().take(15).map(|(id, _)| id) {
        let name = if let Some(user) = ctx.cache.get_user(UserId(id)) {
            user.username.clone()
        } else {
            String::from("Unknown user")
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
    let data = BGRankingEmbed::new(author_idx, initial_scores, global, 1, (1, pages));

    // Creating the embed
    let embed = data.build().build();
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
    let pagination = BGRankingPagination::new(ctx.clone(), response, author_idx, scores, global);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}
