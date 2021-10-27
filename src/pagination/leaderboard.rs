use super::{Pages, Pagination};
use crate::{custom_client::ScraperScore, embeds::LeaderboardEmbed, BotResult};

use rosu_v2::{
    model::beatmap::{Beatmap, BeatmapsetCompact},
    prelude::Username,
};
use twilight_model::channel::Message;

pub struct LeaderboardPagination {
    msg: Message,
    pages: Pages,
    map: Beatmap,
    mapset: Option<BeatmapsetCompact>,
    scores: Vec<ScraperScore>,
    author_name: Option<Username>,
    first_place_icon: Option<String>,
}

impl LeaderboardPagination {
    pub fn new(
        msg: Message,
        map: Beatmap,
        mapset: Option<BeatmapsetCompact>,
        scores: Vec<ScraperScore>,
        author_name: Option<Username>,
        first_place_icon: Option<String>,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(10, scores.len()),
            map,
            mapset,
            scores,
            author_name,
            first_place_icon,
        }
    }
}

#[async_trait]
impl Pagination for LeaderboardPagination {
    type PageData = LeaderboardEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page);

        LeaderboardEmbed::new(
            self.author_name.as_deref(),
            &self.map,
            self.mapset.as_ref(),
            Some(scores),
            &self.first_place_icon,
            self.pages.index,
        )
        .await
    }
}
