use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use rosu::models::{Beatmap, Score, User};
use serenity::{async_trait, cache::Cache};
use std::sync::Arc;

pub struct NoChokePagination {
    pages: Pages,
    user: Box<User>,
    scores: Vec<(usize, Score, Score, Beatmap)>,
    unchoked_pp: f64,
    cache: Arc<Cache>,
}

impl NoChokePagination {
    pub fn new(
        user: User,
        scores: Vec<(usize, Score, Score, Beatmap)>,
        unchoked_pp: f64,
        cache: Arc<Cache>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            user: Box::new(user),
            scores,
            unchoked_pp,
            cache,
        }
    }
}

#[async_trait]
impl Pagination for NoChokePagination {
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<BasicEmbedData, Error> {
        BasicEmbedData::create_nochoke(
            &*self.user,
            self.scores.iter().skip(self.index()).take(self.per_page()),
            self.unchoked_pp,
            (self.page(), self.total_pages()),
            &self.cache,
        )
        .await
    }
}
