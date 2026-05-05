use std::num::NonZeroU8;

use bathbot_model::{ArchivedOsekaiRankingEntry, RankingEntries, RankingEntry, RankingKind};
use bathbot_util::{Authored, constants::GENERAL_ISSUE};
use eyre::{Report, Result};

use crate::{
    Context,
    active::{ActiveMessages, impls::RankingPagination},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub(super) async fn count(
    command: InteractionCommand,
    ranking_kind: &str,
    ranking_options_kind: Option<&str>,
    data: RankingKind,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> u64,
) -> Result<()> {
    let ranking_fut = Context::redis().osekai_ranking(
        ranking_kind,
        ranking_options_kind,
        country(&data),
        NonZeroU8::MIN,
    );

    let ranking = match ranking_fut.await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached osekai ranking"));
        }
    };

    let entries = ranking
        .data
        .iter()
        .map(|entry| RankingEntry {
            value: value_fn(entry),
            name: entry.username.as_str().into(),
            country: Some(entry.country_code.as_str().into()),
        })
        .enumerate()
        .collect();

    let entries = RankingEntries::Amount(entries);

    send_response(command, entries, ranking.max.to_native(), data).await
}

pub(super) async fn pp(
    command: InteractionCommand,
    ranking_kind: &str,
    ranking_options_kind: Option<&str>,
    data: RankingKind,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> f32,
) -> Result<()> {
    let ranking_fut = Context::redis().osekai_ranking(
        ranking_kind,
        ranking_options_kind,
        country(&data),
        NonZeroU8::MIN,
    );

    let ranking = match ranking_fut.await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached osekai ranking"));
        }
    };

    let entries = ranking
        .data
        .iter()
        .map(|entry| RankingEntry {
            value: value_fn(entry),
            name: entry.username.as_str().into(),
            country: Some(entry.country_code.as_str().into()),
        })
        .enumerate()
        .collect();

    let entries = RankingEntries::PpF32(entries);

    send_response(command, entries, ranking.max.to_native(), data).await
}

fn country(kind: &RankingKind) -> Option<&str> {
    match kind {
        RankingKind::OsekaiReplays { country }
        | RankingKind::OsekaiTotalPp { country }
        | RankingKind::OsekaiStandardDeviation { country }
        | RankingKind::OsekaiBadges { country }
        | RankingKind::OsekaiRankedMapsets { country }
        | RankingKind::OsekaiLovedMapsets { country }
        | RankingKind::OsekaiSubscribers { country } => country.as_deref(),
        _ => None,
    }
}

async fn send_response(
    mut command: InteractionCommand,
    entries: RankingEntries,
    total: u32,
    data: RankingKind,
) -> Result<()> {
    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total as usize)
        .kind(data)
        .defer(false)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}
