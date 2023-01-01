use bathbot_macros::pagination;
use eyre::{Result, WrapErr};
use hashbrown::HashMap;
use rosu_pp::Beatmap;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::Difference,
    core::Context,
    custom_client::SnipeRecent,
    embeds::{EmbedData, SnipedDiffEmbed},
    manager::redis::{osu::User, RedisData},
    util::hasher::IntHasher,
};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct SnipedDiffPagination {
    user: RedisData<User>,
    diff: Difference,
    scores: Vec<SnipeRecent>,
    maps: HashMap<u32, Beatmap, IntHasher>,
}

impl SnipedDiffPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let maps = &mut self.maps;

        SnipedDiffEmbed::new(&self.user, self.diff, &self.scores, pages, maps, ctx)
            .await
            .map(EmbedData::build)
            .wrap_err("failed to create embed data")
    }
}
