use super::{Pages, Pagination};
use crate::{
    bail,
    custom_client::{SnipeScore, SnipeScoreParams},
    embeds::PlayerSnipeListEmbed,
    unwind_error, BotResult, Context,
};

use async_trait::async_trait;
use rosu::model::{Beatmap, User};
use std::{
    collections::{BTreeMap, HashMap},
    iter::Extend,
    sync::Arc,
};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct PlayerSnipeListPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: BTreeMap<usize, SnipeScore>,
    maps: HashMap<u32, Beatmap>,
    total: usize,
    params: SnipeScoreParams,
    ctx: Arc<Context>,
}

impl PlayerSnipeListPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: BTreeMap<usize, SnipeScore>,
        maps: HashMap<u32, Beatmap>,
        total: usize,
        params: SnipeScoreParams,
    ) -> Self {
        Self {
            pages: Pages::new(5, total),
            msg,
            user,
            scores,
            maps,
            total,
            params,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for PlayerSnipeListPagination {
    type PageData = PlayerSnipeListEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn reactions() -> Vec<RequestReactionType> {
        Self::arrow_reactions_full()
    }
    fn single_step(&self) -> usize {
        5
    }
    fn multi_step(&self) -> usize {
        25
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

        let embed_fut = PlayerSnipeListEmbed::new(
            &self.user,
            &self.scores,
            &self.maps,
            self.total,
            (self.page(), self.pages.total_pages),
        );
        // .finish or something to store maps
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
