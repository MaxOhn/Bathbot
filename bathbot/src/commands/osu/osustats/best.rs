use bathbot_model::ArchivedOsuStatsBestScores;
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rkyv::{
    boxed::ArchivedBox,
    munge::munge,
    rancor::{Panic, ResultExt},
};
use rosu_v2::prelude::GameMode;

use super::{OsuStatsBest, OsuStatsBestSort};
use crate::{
    active::{ActiveMessages, impls::OsuStatsBestPagination},
    core::{Context, commands::CommandOrigin},
};

pub(super) async fn recentbest(orig: CommandOrigin<'_>, args: OsuStatsBest) -> Result<()> {
    let mode = args.mode.map(GameMode::from).unwrap_or(GameMode::Osu);
    let scores_fut = Context::redis().osustats_best(args.timeframe, mode);

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err));
        }
    };

    let sort = args.sort.unwrap_or_default();

    scores.mutate(|scores| {
        munge!(let ArchivedOsuStatsBestScores { scores, .. } = scores);
        let scores = ArchivedBox::get_seal(scores);

        // SAFETY: We only sort; data is only moved within the reference and no
        // uninitialized bytes will be written.
        let scores = unsafe { scores.unseal_unchecked() };

        match sort {
            OsuStatsBestSort::Accuracy => scores.sort_unstable_by(|a, b| {
                b.accuracy
                    .to_native()
                    .total_cmp(&a.accuracy.to_native())
                    .then_with(|| {
                        a.ended_at
                            .try_deserialize::<Panic>()
                            .always_ok()
                            .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                    })
            }),
            OsuStatsBestSort::Combo => scores.sort_unstable_by(|a, b| {
                b.max_combo.cmp(&a.max_combo).then_with(|| {
                    a.ended_at
                        .try_deserialize::<Panic>()
                        .always_ok()
                        .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                })
            }),
            OsuStatsBestSort::Date => scores.sort_unstable_by_key(|score| {
                score.ended_at.try_deserialize::<Panic>().always_ok()
            }),
            OsuStatsBestSort::LeaderboardPosition => scores.sort_unstable_by(|a, b| {
                a.position.cmp(&b.position).then_with(|| {
                    a.ended_at
                        .try_deserialize::<Panic>()
                        .always_ok()
                        .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                })
            }),
            OsuStatsBestSort::Misses => scores.sort_unstable_by(|a, b| {
                b.count_miss.cmp(&a.count_miss).then_with(|| {
                    a.ended_at
                        .try_deserialize::<Panic>()
                        .always_ok()
                        .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                })
            }),
            OsuStatsBestSort::Pp => scores.sort_unstable_by(|a, b| {
                b.pp.to_native().total_cmp(&a.pp.to_native()).then_with(|| {
                    a.ended_at
                        .try_deserialize::<Panic>()
                        .always_ok()
                        .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                })
            }),
            OsuStatsBestSort::Score => scores.sort_unstable_by(|a, b| {
                b.score.cmp(&a.score).then_with(|| {
                    a.ended_at
                        .try_deserialize::<Panic>()
                        .always_ok()
                        .cmp(&b.ended_at.try_deserialize::<Panic>().always_ok())
                })
            }),
        }
    });

    let pagination = OsuStatsBestPagination::builder()
        .scores(scores)
        .mode(mode)
        .sort(sort)
        .msg_owner(orig.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}
