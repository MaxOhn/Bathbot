use super::{Pages, Pagination};

use crate::{embeds::RecentListEmbed, unwind_error, BotResult, Context};

use async_trait::async_trait;
use rosu::model::{Beatmap, Score, User};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use twilight_model::channel::Message;

pub struct RecentListPagination {
    ctx: Arc<Context>,
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
    maps: HashMap<u32, Beatmap>,
    maps_in_db: HashSet<u32>,
}

impl RecentListPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<Score>,
        maps: HashMap<u32, Beatmap>,
        maps_in_db: HashSet<u32>,
    ) -> Self {
        Self {
            ctx,
            msg,
            user,
            pages: Pages::new(10, scores.len()),
            scores,
            maps,
            maps_in_db,
        }
    }
}

#[async_trait]
impl Pagination for RecentListPagination {
    type PageData = RecentListEmbed;

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

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        // Put missing maps into DB
        if self.maps.len() > self.maps_in_db.len() {
            let map_ids = &self.maps_in_db;

            let maps: Vec<_> = self
                .maps
                .into_iter()
                .filter(|(id, _)| !map_ids.contains(&id))
                .map(|(_, map)| map)
                .collect();

            match ctx.psql().insert_beatmaps(&maps).await {
                Ok(n) if n < 2 => {}
                Ok(n) => info!("Added {} maps to DB", n),
                Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
            }
        }

        Ok(())
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        // Prepare the maps
        for score in self.scores.iter().skip(self.pages.index).take(10) {
            let map_id = score.beatmap_id.unwrap();

            // Make sure map is ready
            #[allow(clippy::clippy::map_entry)]
            if !self.maps.contains_key(&map_id) {
                let map = self
                    .ctx
                    .osu()
                    .beatmap()
                    .map_id(score.beatmap_id.unwrap())
                    .await?
                    .unwrap();

                self.maps.insert(map_id, map);
            }
        }

        let scores = self.scores.iter().skip(self.pages.index).take(10);

        // Create embed data
        RecentListEmbed::new(
            &self.user,
            &self.maps,
            scores,
            (self.page(), self.pages.total_pages),
        )
        .await
    }
}
