use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, scraper::ScraperScore, Error};

use rosu::models::Beatmap;
use serenity::{
    async_trait,
    cache::Cache,
    prelude::{RwLock, TypeMap},
};
use std::sync::Arc;

pub struct LeaderboardPagination {
    pages: Pages,
    map: Box<Beatmap>,
    scores: Vec<ScraperScore>,
    author_name: Option<String>,
    first_place_icon: Option<String>,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
}

impl LeaderboardPagination {
    pub fn new(
        map: Beatmap,
        scores: Vec<ScraperScore>,
        author_name: Option<String>,
        first_place_icon: Option<String>,
        cache: Arc<Cache>,
        data: Arc<RwLock<TypeMap>>,
    ) -> Self {
        Self {
            pages: Pages::new(10, scores.len()),
            map: Box::new(map),
            scores,
            author_name,
            first_place_icon,
            cache,
            data,
        }
    }
}

#[async_trait]
impl Pagination for LeaderboardPagination {
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<BasicEmbedData, Error> {
        BasicEmbedData::create_leaderboard(
            &self.author_name.as_deref(),
            &*self.map,
            Some(self.scores.iter().skip(self.index()).take(self.per_page())),
            &self.first_place_icon,
            self.index(),
            (&self.cache, &self.data),
        )
        .await
    }
}
