use bathbot_macros::pagination;
use bathbot_model::OsuTrackerMapsetEntry;
use bathbot_util::IntHasher;
use eyre::Result;
use hashbrown::HashMap;
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::MapsetEntry,
    core::Context,
    embeds::{EmbedData, OsuTrackerMapsetsEmbed},
};

use super::Pages;

#[pagination(per_page = 10, entries = "entries")]
pub struct OsuTrackerMapsetsPagination {
    entries: Vec<OsuTrackerMapsetEntry>,
    mapsets: HashMap<u32, MapsetEntry, IntHasher>,
}

impl OsuTrackerMapsetsPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let idx = pages.index;
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page)];

        for entry in entries {
            let mapset_id = entry.mapset_id;

            if self.mapsets.contains_key(&mapset_id) {
                continue;
            }

            let mapset = ctx.osu_map().mapset(mapset_id).await?;

            let entry = MapsetEntry {
                creator: mapset.creator.into(),
                name: format!("{} - {}", mapset.artist, mapset.title),
                mapset_id,
                ranked_date: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
                user_id: mapset.user_id as u32,
            };

            self.mapsets.insert(mapset_id, entry);
        }

        Ok(OsuTrackerMapsetsEmbed::new(entries, &self.mapsets, pages).build())
    }
}
