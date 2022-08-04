use command_macros::pagination;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::Beatmapset;
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::MapsetEntry,
    core::Context,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{EmbedData, OsuTrackerMapsetsEmbed},
    util::hasher::SimpleBuildHasher,
    BotResult,
};

use super::Pages;

#[pagination(per_page = 10, entries = "entries")]
pub struct OsuTrackerMapsetsPagination {
    entries: Vec<OsuTrackerMapsetEntry>,
    mapsets: HashMap<u32, MapsetEntry, SimpleBuildHasher>,
}

impl OsuTrackerMapsetsPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> BotResult<Embed> {
        let idx = pages.index;
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page)];

        for entry in entries {
            let mapset_id = entry.mapset_id;

            if self.mapsets.contains_key(&mapset_id) {
                continue;
            }

            let mapset_fut = ctx.psql().get_beatmapset::<Beatmapset>(mapset_id);

            let mapset = match mapset_fut.await {
                Ok(mapset) => mapset,
                Err(_) => {
                    let mapset = ctx.osu().beatmapset(mapset_id).await?;

                    if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                        warn!("{:?}", Report::new(err));
                    }

                    mapset
                }
            };

            let entry = MapsetEntry {
                creator: mapset.creator_name,
                name: format!("{} - {}", mapset.artist, mapset.title),
                mapset_id,
                ranked_date: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
                user_id: mapset.creator_id,
            };

            self.mapsets.insert(mapset_id, entry);
        }

        Ok(OsuTrackerMapsetsEmbed::new(entries, &self.mapsets, pages).build())
    }
}
