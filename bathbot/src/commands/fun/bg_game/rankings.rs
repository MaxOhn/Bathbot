use std::{collections::BTreeMap, sync::Arc};

use eyre::Result;
use hashbrown::HashSet;
use twilight_model::{channel::Message, id::Id};

use crate::{
    embeds::{RankingEntries, RankingEntry, RankingKind},
    pagination::RankingPagination,
    util::{constants::GENERAL_ISSUE, hasher::IntHasher, ChannelExt},
    Context,
};

pub async fn leaderboard(ctx: Arc<Context>, msg: &Message, global: bool) -> Result<()> {
    let mut scores = match ctx.games().bggame_leaderboard().await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get bggame scores"));
        }
    };

    let guild = msg.guild_id;

    if let Some(guild) = guild.filter(|_| !global) {
        let members: HashSet<_, IntHasher> = ctx.cache.members(guild, |id| id.get() as i64);
        scores.retain(|row| members.contains(&row.discord_id));
    }

    let author = msg.author.id.get() as i64;

    scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
    let author_idx = scores.iter().position(|row| row.discord_id == author);

    // Gather usernames for initial page
    let mut entries = BTreeMap::new();

    for (i, row) in scores.iter().enumerate().take(20) {
        let id = Id::new(row.discord_id as u64);

        let name = match ctx.user_config().osu_name(id).await {
            Ok(Some(name)) => name,
            Ok(None) => ctx
                .cache
                .user(id, |user| user.name.as_str().into())
                .unwrap_or_else(|_| "<unknown user>".into()),
            Err(err) => {
                warn!("{err:?}");

                "<unknown user>".into()
            }
        };

        let entry = RankingEntry {
            value: row.score as u64,
            name,
            country: None,
        };

        entries.insert(i, entry);
    }

    let entries = RankingEntries::Amount(entries);

    // Prepare initial page
    let total = scores.len();
    let global = guild.is_none() || global;
    let data = RankingKind::BgScores { global, scores };

    RankingPagination::builder(entries, total, author_idx, data)
        .start(ctx, msg.into())
        .await
}
