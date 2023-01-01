use bathbot_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{OsuTrackerCountryDetailsCompact, ScoreOrder},
    custom_client::OsuTrackerCountryScore,
    embeds::{EmbedData, OsuTrackerCountryTopEmbed},
};

use super::Pages;

#[pagination(per_page = 10, entries = "scores")]
pub struct OsuTrackerCountryTopPagination {
    details: OsuTrackerCountryDetailsCompact,
    scores: Vec<(OsuTrackerCountryScore, usize)>,
    sort_by: ScoreOrder,
}

impl OsuTrackerCountryTopPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let scores = &self.scores[idx..self.scores.len().min(idx + pages.per_page)];

        OsuTrackerCountryTopEmbed::new(&self.details, scores, self.sort_by, pages).build()
    }
}
