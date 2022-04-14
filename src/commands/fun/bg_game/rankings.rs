use std::sync::Arc;

use eyre::Report;
use hashbrown::{HashMap, HashSet};
use twilight_model::{channel::Message, id::Id};

use crate::{
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, numbers, ChannelExt},
    BotResult, Context,
};

pub async fn leaderboard(ctx: Arc<Context>, msg: &Message, global: bool) -> BotResult<()> {
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let guild = msg.guild_id;

    if let Some(guild) = guild.filter(|_| !global) {
        let members: HashSet<_> = ctx.cache.members(guild, |id| id.get());
        scores.retain(|(id, _)| members.contains(id));
    }

    let author = msg.author.id.get();

    scores.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
    let author_idx = scores.iter().position(|(user, _)| *user == author);

    // Gather usernames for initial page
    let mut usernames = HashMap::with_capacity(15);

    for &id in scores.iter().take(15).map(|(id, _)| id) {
        let user_id = Id::new(id);

        let name = ctx
            .cache
            .user(user_id, |user| user.name.clone())
            .unwrap_or_else(|_| "Unknown user".to_owned());

        usernames.insert(id, name);
    }

    let initial_scores = scores
        .iter()
        .take(15)
        .map(|(id, score)| (&usernames[id], *score))
        .collect();

    // Prepare initial page
    let pages = numbers::div_euclid(15, scores.len());
    let global = guild.is_none() || global;
    let embed_data = BGRankingEmbed::new(author_idx, initial_scores, 1, global, (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = msg.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 15 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = BGRankingPagination::new(
        Arc::clone(&ctx),
        response,
        author_idx,
        scores,
        usernames,
        global,
    );

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
