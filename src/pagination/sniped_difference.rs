use command_macros::pagination;
use hashbrown::HashMap;
use rosu_pp::Beatmap;
use rosu_v2::model::user::User;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::Difference,
    core::Context,
    custom_client::SnipeRecent,
    embeds::{EmbedData, SnipedDiffEmbed},
    util::hasher::SimpleBuildHasher,
    BotResult,
};

use super::Pages;

#[pagination(per_page = 5, entries = "scores")]
pub struct SnipedDiffPagination {
    user: User,
    diff: Difference,
    scores: Vec<SnipeRecent>,
    maps: HashMap<u32, Beatmap, SimpleBuildHasher>,
}

impl SnipedDiffPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> BotResult<Embed> {
        let embed_fut = SnipedDiffEmbed::new(
            &self.user,
            self.diff,
            &self.scores,
            pages,
            &mut self.maps,
            ctx,
        );

        embed_fut.await.map(EmbedData::build)
    }
}
