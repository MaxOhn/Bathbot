use command_macros::pagination;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    embeds::{EmbedData, RecentListEmbed},
    Context,
};

use super::Pages;

#[pagination(per_page = 10, entries = "scores")]
pub struct RecentListPagination {
    user: User,
    scores: Vec<Score>,
}

impl RecentListPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let scores = self.scores.iter().skip(pages.index).take(pages.per_page);

        RecentListEmbed::new(&self.user, scores, ctx, pages)
            .await
            .map(EmbedData::build)
            .wrap_err("failed to create embed data")
    }
}
