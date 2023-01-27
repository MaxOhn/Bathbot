use std::collections::HashMap;

use bathbot_macros::pagination;
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use rosu_pp::DifficultyAttributes;
use rosu_v2::prelude::Score;
use twilight_model::channel::embed::Embed;

use crate::{
    embeds::{EmbedData, RecentListEmbed},
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    Context,
};

use super::Pages;

#[pagination(per_page = 10, entries = "scores")]
pub struct RecentListPagination {
    user: RedisData<User>,
    scores: Vec<Score>,
    maps: HashMap<u32, OsuMap, IntHasher>,
    attr_map: HashMap<(u32, u32), (DifficultyAttributes, f32)>,
}

impl RecentListPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let end_idx = self.scores.len().min(pages.index() + pages.per_page());
        let scores = &self.scores[pages.index()..end_idx];

        let missing_maps: HashMap<_, _, _> = scores
            .iter()
            .filter(|score| !self.maps.contains_key(&score.map_id))
            .map(|score| {
                (
                    score.map_id as i32,
                    score.map.as_ref().and_then(|map| map.checksum.as_deref()),
                )
            })
            .collect();

        if !missing_maps.is_empty() {
            let missing_maps = ctx
                .osu_map()
                .maps(&missing_maps)
                .await
                .wrap_err("failed to extend missing maps")?;

            self.maps.extend(missing_maps);
        }

        let embed_fut = RecentListEmbed::new(
            &self.user,
            scores,
            &self.maps,
            &mut self.attr_map,
            ctx,
            pages,
        );

        Ok(embed_fut.await.build())
    }
}
