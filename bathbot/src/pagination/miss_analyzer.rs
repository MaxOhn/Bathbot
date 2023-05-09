use eyre::{Report, Result};
use twilight_model::channel::message::Embed;

use super::{Pages, PaginationBuilder, PaginationKind};

pub struct MissAnalyzerPagination {
    pub score_id: u64,
    initial_embed: Option<Embed>,
    pub edited_embed: Option<Embed>,
    pub content: Option<String>,
}

impl MissAnalyzerPagination {
    pub fn builder(
        score_id: u64,
        initial_embed: Embed,
        edited_embed: Option<Embed>,
        content: Option<String>,
    ) -> PaginationBuilder {
        let kind = Self {
            score_id,
            initial_embed: Some(initial_embed),
            edited_embed,
            content,
        };
        let pages = Pages::new(1, 3); // pagination only starts when there's more than one page

        PaginationBuilder::new(PaginationKind::MissAnalyzer(Box::new(kind)), pages)
    }

    pub fn build_page(&mut self) -> Result<Embed> {
        self.initial_embed
            .take()
            .ok_or_else(|| Report::msg("Already used miss analyzer embed"))
    }
}
