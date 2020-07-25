use super::{Pages, Pagination};

use crate::{custom_client::MostPlayedMap, embeds::MostPlayedCommonEmbed, BotResult};

use async_trait::async_trait;
use rosu::models::User;
use std::collections::HashMap;
use twilight::model::{channel::Message, id::UserId};

pub struct MostPlayedCommonPagination {
    msg: Message,
    pages: Pages,
    users: HashMap<u32, User>,
    users_count: HashMap<u32, HashMap<u32, u32>>,
    maps: Vec<MostPlayedMap>,
    thumbnail: String,
}

impl MostPlayedCommonPagination {
    pub fn new(
        msg: Message,
        users: HashMap<u32, User>,
        users_count: HashMap<u32, HashMap<u32, u32>>,
        maps: Vec<MostPlayedMap>,
        thumbnail: String,
    ) -> Self {
        Self {
            pages: Pages::new(10, maps.len()),
            msg,
            users,
            users_count,
            maps,
            thumbnail,
        }
    }
}

#[async_trait]
impl Pagination for MostPlayedCommonPagination {
    type PageData = MostPlayedCommonEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn thumbnail(&self) -> Option<String> {
        Some(self.thumbnail.clone())
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        Ok(MostPlayedCommonEmbed::new(
            &self.users,
            &self.maps[self.pages.index..(self.pages.index + 10).min(self.maps.len())],
            &self.users_count,
            self.pages.index,
        )
        .await)
    }
}
