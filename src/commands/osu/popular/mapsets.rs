use std::sync::Arc;

use eyre::{Report, Result};
use hashbrown::HashMap;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{Beatmapset, Username};
use time::OffsetDateTime;

use crate::{
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    pagination::OsuTrackerMapsetsPagination,
    util::{
        constants::{OSUTRACKER_ISSUE, OSU_API_ISSUE},
        hasher::IntHasher,
        interaction::InteractionCommand,
        InteractionCommandExt,
    },
};

pub(super) async fn mapsets(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let mut counts: Vec<OsuTrackerMapsetEntry> = match ctx.redis().osutracker_stats().await {
        Ok(stats) => stats
            .get()
            .mapset_count
            .deserialize(&mut Infallible)
            .unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osutracker stats"));
        }
    };

    counts.truncate(727);

    let mut mapsets = HashMap::with_hasher(IntHasher);

    for entry in counts.iter().take(10) {
        let mapset_id = entry.mapset_id;

        let mapset = match ctx.psql().get_beatmapset::<Beatmapset>(mapset_id).await {
            Ok(mapset) => mapset,
            Err(_) => match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => {
                    if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                        warn!("{:?}", err.wrap_err("Failed to insert mapset in database"));
                    }

                    mapset
                }
                Err(err) => {
                    let _ = command.error(&ctx, OSU_API_ISSUE).await;
                    let report = Report::new(err).wrap_err("failed to get beatmapset");

                    return Err(report);
                }
            },
        };

        let entry = MapsetEntry {
            creator: mapset.creator_name,
            name: format!("{} - {}", mapset.artist, mapset.title),
            mapset_id,
            ranked_date: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
            user_id: mapset.creator_id,
        };

        mapsets.insert(mapset_id, entry);
    }

    OsuTrackerMapsetsPagination::builder(counts, mapsets)
        .start_by_update()
        .defer_components()
        .start(ctx, (&mut command).into())
        .await
}

pub struct MapsetEntry {
    pub creator: Username,
    pub name: String,
    pub mapset_id: u32,
    pub ranked_date: OffsetDateTime,
    pub user_id: u32,
}
