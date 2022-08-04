use std::sync::Arc;

use eyre::Report;
use hashbrown::HashMap;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{Beatmapset, Username};
use time::OffsetDateTime;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::{commands::CommandOrigin, Context},
    custom_client::OsuTrackerMapsetEntry,
    pagination::OsuTrackerMapsetsPagination,
    util::{
        constants::{OSUTRACKER_ISSUE, OSU_API_ISSUE},
        hasher::SimpleBuildHasher,
        ApplicationCommandExt,
    },
    BotResult,
};

pub(super) async fn mapsets(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let mut counts: Vec<OsuTrackerMapsetEntry> = match ctx.redis().osutracker_stats().await {
        Ok(stats) => stats
            .get()
            .mapset_count
            .deserialize(&mut Infallible)
            .unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    counts.truncate(727);

    let mut mapsets = HashMap::with_hasher(SimpleBuildHasher);

    for entry in counts.iter().take(10) {
        let mapset_id = entry.mapset_id;

        let mapset = match ctx.psql().get_beatmapset::<Beatmapset>(mapset_id).await {
            Ok(mapset) => mapset,
            Err(_) => match ctx.osu().beatmapset(mapset_id).await {
                Ok(mapset) => {
                    if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                        warn!("{:?}", Report::new(err));
                    }

                    mapset
                }
                Err(err) => {
                    let _ = command.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
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
        .start(ctx, CommandOrigin::Interaction { command })
        .await
}

pub struct MapsetEntry {
    pub creator: Username,
    pub name: String,
    pub mapset_id: u32,
    pub ranked_date: OffsetDateTime,
    pub user_id: u32,
}
