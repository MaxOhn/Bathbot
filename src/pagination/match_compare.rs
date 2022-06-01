use command_macros::pagination;
use twilight_model::channel::embed::Embed;

use super::Pages;

#[pagination(per_page = 1, entries = "embeds")]
pub struct MatchComparePagination {
    embeds: Vec<Embed>,
}

impl MatchComparePagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        self.embeds[pages.index].clone()
    }
}
