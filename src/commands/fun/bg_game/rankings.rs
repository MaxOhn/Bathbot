use std::{collections::BTreeMap, sync::Arc};

use eyre::Report;
use hashbrown::HashSet;
use twilight_model::{channel::Message, id::Id};

use crate::{
    commands::osu::UserValue,
    embeds::{RankingEntry, RankingKindData},
    pagination::RankingPagination,
    util::{constants::GENERAL_ISSUE, ChannelExt},
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
    let mut users = BTreeMap::new();

    for (i, (id, score)) in scores.iter().enumerate().take(20) {
        let id = Id::new(*id);

        let name = match ctx.psql().get_user_osu(id).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => ctx
                .cache
                .user(id, |user| user.name.clone())
                .unwrap_or_else(|_| "Unknown user".to_owned())
                .into(),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to get osu user");
                warn!("{report:?}");

                ctx.cache
                    .user(id, |user| user.name.clone())
                    .unwrap_or_else(|_| "Unknown user".to_owned())
                    .into()
            }
        };

        let entry = RankingEntry {
            value: UserValue::Amount(*score as u64),
            name,
            country: None,
        };

        users.insert(i, entry);
    }

    // Prepare initial page
    let total = scores.len();
    let global = guild.is_none() || global;
    let data = RankingKindData::BgScores { global, scores };

    RankingPagination::builder(Arc::clone(&ctx), users, total, author_idx, data)
        .start(ctx, msg.into())
        .await
}
