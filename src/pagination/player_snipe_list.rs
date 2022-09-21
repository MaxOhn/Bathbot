use std::{collections::BTreeMap, iter::Extend};

use command_macros::pagination;
use eyre::{Result, WrapErr};
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, User};
use twilight_model::channel::embed::Embed;

use crate::{
    custom_client::{SnipeScore, SnipeScoreParams},
    embeds::{EmbedData, PlayerSnipeListEmbed},
    util::hasher::IntHasher,
    Context,
};

use super::Pages;

#[pagination(per_page = 5, total = "total")]
pub struct PlayerSnipeListPagination {
    user: User,
    scores: BTreeMap<usize, SnipeScore>,
    maps: HashMap<u32, Beatmap, IntHasher>,
    total: usize,
    params: SnipeScoreParams,
}

impl PlayerSnipeListPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let count = self
            .scores
            .range(pages.index..pages.index + pages.per_page)
            .count();

        if count < pages.per_page && count < self.total - pages.index {
            let huismetbenen_page = pages.index / 50;
            self.params.page(huismetbenen_page as u8);

            // Get scores
            let scores = ctx
                .client()
                .get_national_firsts(&self.params)
                .await
                .wrap_err("failed to get national firsts")?;

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
            .range(pages.index..pages.index + pages.per_page)
            .map(|(_, score)| score.map_id)
            .filter(|map_id| !self.maps.contains_key(map_id))
            .map(|id| id as i32)
            .collect();

        if !map_ids.is_empty() {
            let mut maps = match ctx.psql().get_beatmaps(&map_ids, true).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!("{:?}", err.wrap_err("Failed to get maps from database"));

                    HashMap::default()
                }
            };

            // Get missing maps from API
            for map_id in map_ids {
                let map_id = map_id as u32;

                if !maps.contains_key(&map_id) {
                    match ctx.osu().beatmap().map_id(map_id).await {
                        Ok(map) => {
                            maps.insert(map_id, map);
                        }
                        Err(err) => return Err(err.into()),
                    }
                }
            }

            self.maps.extend(maps);
        }

        let embed_fut =
            PlayerSnipeListEmbed::new(&self.user, &self.scores, &self.maps, self.total, ctx, pages);

        Ok(embed_fut.await.build())
    }
}
