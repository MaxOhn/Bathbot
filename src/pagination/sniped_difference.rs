use super::{Pages, Pagination};
use crate::{
    bail, commands::osu::Difference, custom_client::SnipeRecent, embeds::SnipedDiffEmbed,
    unwind_error, BotResult, Context,
};

use async_trait::async_trait;
use rosu::model::{Beatmap, User};
use std::{collections::HashMap, iter::Extend, sync::Arc};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct SnipedDiffPagination {
    msg: Message,
    pages: Pages,
    user: User,
    diff: Difference,
    scores: Vec<SnipeRecent>,
    maps: HashMap<u32, Beatmap>,
    ctx: Arc<Context>,
}

impl SnipedDiffPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        diff: Difference,
        scores: Vec<SnipeRecent>,
        maps: HashMap<u32, Beatmap>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            msg,
            user,
            diff,
            scores,
            maps,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for SnipedDiffPagination {
    type PageData = SnipedDiffEmbed;

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
        let entries = self
            .scores
            .range(self.pages.index..self.pages.index + self.pages.per_page);

        let count = entries.count();

        if count < self.pages.per_page && self.total - self.pages.index > count {
            let huismetbenen_page = self.pages.index / 50;
            self.params.page(huismetbenen_page as u8);

            // Get scores
            let scores = self
                .ctx
                .clients
                .custom
                .get_national_firsts(&self.params)
                .await?;

            // Store scores in BTreeMap
            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| (huismetbenen_page * 50 + i, s));

            self.scores.extend(iter);
        }

        // Get maps from DB
        let map_ids: Vec<_> = self
            .scores
            .range(self.pages.index..self.pages.index + self.pages.per_page)
            .map(|(_, score)| score.beatmap_id)
            .filter(|map_id| !self.maps.contains_key(map_id))
            .collect();

        if !map_ids.is_empty() {
            let mut maps = match self.ctx.psql().get_beatmaps(&map_ids).await {
                Ok(maps) => maps,
                Err(why) => {
                    unwind_error!(warn, why, "Error while getting maps from DB: {}");
                    HashMap::default()
                }
            };

            // Get missing maps from API
            for map_id in map_ids {
                if !maps.contains_key(&map_id) {
                    match self.ctx.osu().beatmap().map_id(map_id).await {
                        Ok(Some(map)) => {
                            maps.insert(map_id, map);
                        }
                        Ok(None) => bail!("The API returned no beatmap for map id {}", map_id),
                        Err(why) => return Err(why.into()),
                    }
                }
            }

            self.maps.extend(maps);
        }

        let embed_fut = SnipedDiffEmbed::new(
            &self.user,
            self.diff,
            &self.scores, // TODO
            &self.maps,
            (self.page(), self.pages.total_pages),
        );

        Ok(embed_fut.await)
    }

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        // Put maps into DB
        let maps: Vec<_> = self.maps.into_iter().map(|(_, map)| map).collect();

        match ctx.psql().insert_beatmaps(&maps).await {
            Ok(n) => debug!("Added up to {} maps to DB", n),
            Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
        }

        Ok(())
    }
}
