use std::{collections::BTreeMap, iter::Extend, sync::Arc};

use command_macros::BasePagination;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, User};
use twilight_model::channel::Message;

use crate::{
    custom_client::{SnipeScore, SnipeScoreParams},
    embeds::PlayerSnipeListEmbed,
    BotResult, Context,
};

use super::{Pages, Pagination};

#[derive(BasePagination)]
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

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let count = self
            .scores
            .range(self.pages.index..self.pages.index + self.pages.per_page)
            .count();

        if count < self.pages.per_page && count < self.total - self.pages.index {
            let huismetbenen_page = self.pages.index / 50;
            self.params.page(huismetbenen_page as u8);

            // Get scores
            let scores = self.ctx.client().get_national_firsts(&self.params).await?;

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
            .map(|id| id as i32)
            .collect();

        if !map_ids.is_empty() {
            let mut maps = match self.ctx.psql().get_beatmaps(&map_ids, true).await {
                Ok(maps) => maps,
                Err(err) => {
                    let report = Report::new(err).wrap_err("error while getting maps from DB");
                    warn!("{report:?}");

                    HashMap::default()
                }
            };

            // Get missing maps from API
            for map_id in map_ids {
                let map_id = map_id as u32;

                if !maps.contains_key(&map_id) {
                    match self.ctx.osu().beatmap().map_id(map_id).await {
                        Ok(map) => {
                            maps.insert(map_id, map);
                        }
                        Err(err) => return Err(err.into()),
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
            &self.ctx,
            (self.page(), self.pages.total_pages),
        );

        Ok(embed_fut.await)
    }

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        match ctx.psql().insert_beatmaps(self.maps.values()).await {
            Ok(n) => debug!("Added {n} maps to DB"),
            Err(err) => warn!("{:?}", Report::new(err)),
        }

        Ok(())
    }
}
