use super::{Pages, Pagination};

use crate::{embeds::MostPlayedEmbed, BotResult};

use rosu_v2::prelude::{MostPlayedMap, User};
use twilight_model::channel::Message;

pub struct MostPlayedPagination {
    msg: Message,
    pages: Pages,
    user: Box<User>,
    maps: Vec<MostPlayedMap>,
}

impl MostPlayedPagination {
    pub fn new(msg: Message, user: User, maps: Vec<MostPlayedMap>) -> Self {
        Self {
            msg,
            pages: Pages::new(10, maps.len()),
            user: Box::new(user),
            maps,
        }
    }
}

#[async_trait]
impl Pagination for MostPlayedPagination {
    type PageData = MostPlayedEmbed;

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
        Ok(MostPlayedEmbed::new(
            &*self.user,
            self.maps
                .iter()
                .skip(self.pages.index)
                .take(self.pages.per_page),
            (self.page(), self.pages.total_pages),
        ))
    }
}
