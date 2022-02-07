use std::sync::Arc;

use eyre::Report;
use hashbrown::{HashMap, HashSet};
use twilight_model::id::Id;

use crate::{
    embeds::{BGRankingEmbed, EmbedData},
    pagination::{BGRankingPagination, Pagination},
    util::{constants::GENERAL_ISSUE, numbers, CowUtils, MessageExt},
    BotResult, CommandData, Context,
};

#[command]
#[short_desc("Show the user rankings for the game")]
#[aliases("rankings", "leaderboard", "lb", "stats")]
async fn rankings(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let non_global = args
                .next()
                .filter(|_| msg.guild_id.is_some())
                .map(CowUtils::cow_to_ascii_lowercase)
                .filter(|arg| arg == "server" || arg == "s")
                .is_some();

            _rankings(ctx, CommandData::Message { msg, args, num }, non_global).await
        }
        CommandData::Interaction { .. } => unreachable!(),
    }
}

pub(super) async fn _rankings(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    non_global: bool,
) -> BotResult<()> {
    let mut scores = match ctx.psql().all_bggame_scores().await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let guild_id = data.guild_id();

    if let Some(guild_id) = guild_id.filter(|_| non_global) {
        let members: HashSet<_> = ctx.cache.members(guild_id, |id| id.get());
        scores.retain(|(id, _)| members.contains(id));
    }

    let author_id = data.author()?.id;

    scores.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
    let author_idx = scores.iter().position(|(user, _)| *user == author_id.get());

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
    let global = guild_id.is_none() || !non_global;
    let embed_data = BGRankingEmbed::new(author_idx, initial_scores, 1, global, (1, pages));

    // Creating the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

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

    let owner = author_id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
