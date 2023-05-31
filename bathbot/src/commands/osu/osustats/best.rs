use std::sync::Arc;

use bathbot_util::constants::OSUSTATS_API_ISSUE;
use eyre::Result;
use rosu_v2::prelude::GameMode;

use super::{OsuStatsBest, OsuStatsBestSort};
use crate::{
    active::{impls::OsuStatsBestPagination, ActiveMessages},
    core::{commands::CommandOrigin, Context},
};

pub(super) async fn recentbest(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: OsuStatsBest,
) -> Result<()> {
    let mode = args.mode.map(GameMode::from).unwrap_or(GameMode::Osu);
    let scores_fut = ctx.redis().osustats_best(args.timeframe, mode);

    let mut scores = match scores_fut.await {
        Ok(scores) => scores.into_original(),
        Err(err) => {
            let _ = orig.error(&ctx, OSUSTATS_API_ISSUE).await;

            return Err(err);
        }
    };

    let sort = args.sort.unwrap_or_default();

    match sort {
        OsuStatsBestSort::Accuracy => scores.scores.sort_unstable_by(|a, b| {
            b.accuracy
                .total_cmp(&a.accuracy)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
        OsuStatsBestSort::Combo => scores.scores.sort_unstable_by(|a, b| {
            b.max_combo
                .cmp(&a.max_combo)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
        OsuStatsBestSort::Date => scores.scores.sort_unstable_by_key(|score| score.ended_at),
        OsuStatsBestSort::LeaderboardPosition => scores.scores.sort_unstable_by(|a, b| {
            a.position
                .cmp(&b.position)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
        OsuStatsBestSort::Misses => scores.scores.sort_unstable_by(|a, b| {
            b.count_miss
                .cmp(&a.count_miss)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
        OsuStatsBestSort::Pp => scores.scores.sort_unstable_by(|a, b| {
            b.pp.total_cmp(&a.pp)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
        OsuStatsBestSort::Score => scores.scores.sort_unstable_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.ended_at.cmp(&b.ended_at))
        }),
    }

    let pagination = OsuStatsBestPagination::builder()
        .scores(scores)
        .mode(mode)
        .sort(sort)
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}
