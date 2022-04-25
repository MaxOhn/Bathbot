use super::{Pages, Pagination};

use crate::{embeds::MostPlayedEmbed, BotResult};

use command_macros::BasePagination;
use rosu_v2::prelude::{MostPlayedMap, User};
use twilight_model::channel::Message;

#[derive(BasePagination)]
#[pagination(single_step = 10)]
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
