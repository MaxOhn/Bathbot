use std::sync::Arc;

use chrono::Utc;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::Beatmapset;
use twilight_model::channel::Message;

use crate::{
    commands::osu::MapsetEntry, core::Context, custom_client::OsuTrackerMapsetEntry,
    embeds::OsuTrackerMapsetsEmbed, BotResult,
};

use super::{Pages, Pagination};

pub struct OsuTrackerMapsetsPagination {
    msg: Message,
    pages: Pages,
    entries: Vec<OsuTrackerMapsetEntry>,
    mapsets: HashMap<u32, MapsetEntry>,
    ctx: Arc<Context>,
}

impl OsuTrackerMapsetsPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        entries: Vec<OsuTrackerMapsetEntry>,
        mapsets: HashMap<u32, MapsetEntry>,
    ) -> Self {
        Self {
            ctx,
            pages: Pages::new(10, entries.len()),
            msg,
            entries,
            mapsets,
        }
    }
}

#[async_trait]
impl Pagination for OsuTrackerMapsetsPagination {
    type PageData = OsuTrackerMapsetsEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let index = self.pages.index;
        let entries = &self.entries[index..(index + 10).min(self.entries.len())];

        for entry in entries {
            let mapset_id = entry.mapset_id;

            if self.mapsets.contains_key(&mapset_id) {
                continue;
            }

            let mapset_fut = self.ctx.psql().get_beatmapset::<Beatmapset>(mapset_id);

            let mapset = match mapset_fut.await {
                Ok(mapset) => mapset,
                Err(_) => {
                    let mapset = self.ctx.osu().beatmapset(mapset_id).await?;

                    if let Err(err) = self.ctx.psql().insert_beatmapset(&mapset).await {
                        warn!("{:?}", Report::new(err));
                    }

                    mapset
                }
            };

            let entry = MapsetEntry {
                creator: mapset.creator_name,
                name: format!("{} - {}", mapset.artist, mapset.title),
                mapset_id,
                ranked_date: mapset.ranked_date.unwrap_or_else(Utc::now),
                user_id: mapset.creator_id,
            };

            self.mapsets.insert(mapset_id, entry);
        }

        let page = self.page();
        let pages = self.pages.total_pages;
        let embed = OsuTrackerMapsetsEmbed::new(entries, &self.mapsets, (page, pages));

        Ok(embed)
    }
}
