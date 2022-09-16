use command_macros::pagination;
use eyre::Result;
use rosu_v2::{model::beatmap::Beatmap, prelude::Username};
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    custom_client::ScraperScore,
    embeds::{EmbedData, LeaderboardEmbed},
};

use super::Pages;

#[pagination(per_page = 10, entries = "scores")]
pub struct LeaderboardPagination {
    map: Beatmap,
    scores: Vec<ScraperScore>,
    author_name: Option<Username>,
    first_place_icon: Option<String>,
}

impl LeaderboardPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        let embed_fut = LeaderboardEmbed::new(
            self.author_name.as_deref(),
            &self.map,
            Some(scores),
            &self.first_place_icon,
            ctx,
            pages,
        );

        embed_fut.await.map(EmbedData::build)
    }
}
