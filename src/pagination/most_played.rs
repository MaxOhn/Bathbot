use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, scraper::MostPlayedMap, Error};

use rosu::models::User;
use serenity::async_trait;

pub struct MostPlayedPagination {
    pages: Pages,
    user: Box<User>,
    maps: Vec<MostPlayedMap>,
}

impl MostPlayedPagination {
    pub fn new(user: User, maps: Vec<MostPlayedMap>) -> Self {
        Self {
            pages: Pages::new(10, maps.len()),
            user: Box::new(user),
            maps,
        }
    }
}

#[async_trait]
impl Pagination for MostPlayedPagination {
    type PageData = BasicEmbedData;
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        Ok(BasicEmbedData::create_mostplayed(
            &*self.user,
            self.maps.iter().skip(self.index()).take(self.per_page()),
            (self.page(), self.total_pages()),
        ))
    }
}
