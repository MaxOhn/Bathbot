use super::{Pages, Pagination};

use crate::{custom_client::MostPlayedMap, embeds::MostPlayedEmbed, BotResult};

use async_trait::async_trait;
use rosu::models::User;
use twilight::model::{channel::Message, id::UserId};

pub struct MostPlayedPagination {
    msg: Message,
    pages: Pages,
    user: Box<User>,
    maps: Vec<MostPlayedMap>,
}

impl MostPlayedPagination {
    pub async fn new(msg: Message, user: User, maps: Vec<MostPlayedMap>) -> Self {
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
