use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::{
    async_trait,
    cache::Cache,
    prelude::{RwLock, TypeMap},
};
use std::sync::Arc;

pub struct TopPagination {
    pages: Pages,
    user: Box<User>,
    scores: Vec<(usize, Score, Beatmap)>,
    mode: GameMode,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
}

impl TopPagination {
    pub fn new(
        user: User,
        scores: Vec<(usize, Score, Beatmap)>,
        mode: GameMode,
        cache: Arc<Cache>,
        data: Arc<RwLock<TypeMap>>,
    ) -> Self {
        Self {
            pages: Pages::new(5, scores.len()),
            user: Box::new(user),
            scores,
            mode,
            cache,
            data,
        }
    }
}

#[async_trait]
impl Pagination for TopPagination {
    type PageData = BasicEmbedData;
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        BasicEmbedData::create_top(
            &*self.user,
            self.scores.iter().skip(self.index()).take(self.per_page()),
            self.mode,
            (self.page(), self.total_pages()),
            (&self.cache, &self.data),
        )
        .await
    }
}
