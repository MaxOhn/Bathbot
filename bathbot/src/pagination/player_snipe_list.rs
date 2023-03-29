use std::{
    collections::{BTreeMap, HashMap},
    iter::Extend,
};

use bathbot_macros::pagination;
use bathbot_model::{rosu_v2::user::User, SnipeScore, SnipeScoreParams};
use bathbot_util::IntHasher;
use eyre::{Report, Result, WrapErr};
use twilight_model::channel::message::embed::Embed;

use crate::{
    embeds::{EmbedData, PlayerSnipeListEmbed},
    manager::{redis::RedisData, OsuMap},
    Context,
};

use super::Pages;

#[pagination(per_page = 5, total = "total")]
pub struct PlayerSnipeListPagination {
    user: RedisData<User>,
    scores: BTreeMap<usize, SnipeScore>,
    maps: HashMap<u32, OsuMap, IntHasher>,
    total: usize,
    params: SnipeScoreParams,
}

impl PlayerSnipeListPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let count = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() && count < self.total - pages.index() {
            let huismetbenen_page = pages.index() / 50 + 1;
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
                .map(|(i, s)| ((huismetbenen_page - 1) * 50 + i, s));

            self.scores.extend(iter);
        }

        // Get maps from DB
        let map_ids: HashMap<_, _, _> = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .filter_map(|(_, score)| {
                if self.maps.contains_key(&score.map.map_id) {
                    None
                } else {
                    Some((score.map.map_id as i32, None))
                }
            })
            .collect();

        if !map_ids.is_empty() {
            let new_maps = match ctx.osu_map().maps(&map_ids).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!(
                        "{:?}",
                        Report::new(err).wrap_err("Failed to get maps from database")
                    );

                    HashMap::default()
                }
            };

            self.maps.extend(new_maps);
        }

        PlayerSnipeListEmbed::new(&self.user, &self.scores, &self.maps, self.total, ctx, pages)
            .await
            .map(EmbedData::build)
            .wrap_err("failed to build snipe list embed")
    }
}
