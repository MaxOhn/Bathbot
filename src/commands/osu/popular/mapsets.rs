use std::sync::Arc;

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmapset, Username};

use crate::{
    core::{commands::CommandData, Context},
    embeds::{EmbedData, OsuTrackerMapsetsEmbed},
    pagination::{OsuTrackerMapsetsPagination, Pagination},
    util::{
        constants::{OSUTRACKER_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    BotResult,
};

pub(super) async fn mapsets_(ctx: Arc<Context>, data: CommandData<'_>) -> BotResult<()> {
    let mut counts = match ctx.clients.custom.get_osutracker_stats().await {
        Ok(stats) => stats.mapset_count,
        Err(err) => {
            let _ = data.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    counts.truncate(500);

    let mut mapsets = HashMap::new();

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
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
            },
        };

        let entry = MapsetEntry {
            creator: mapset.creator_name,
            name: format!("{} - {}", mapset.artist, mapset.title),
            mapset_id,
            ranked_date: mapset.ranked_date.unwrap_or_else(Utc::now),
            user_id: mapset.creator_id,
        };

        mapsets.insert(mapset_id, entry);
    }

    let pages = numbers::div_euclid(10, counts.len());
    let initial = &counts[..counts.len().min(10)];

    let embed = OsuTrackerMapsetsEmbed::new(initial, &mapsets, (1, pages))
        .into_builder()
        .build();

    let response_raw = data.create_message(&ctx, embed.into()).await?;

    if counts.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerMapsetsPagination::new(Arc::clone(&ctx), response, counts, mapsets);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub struct MapsetEntry {
    pub creator: Username,
    pub name: String,
    pub mapset_id: u32,
    pub ranked_date: DateTime<Utc>,
    pub user_id: u32,
}
