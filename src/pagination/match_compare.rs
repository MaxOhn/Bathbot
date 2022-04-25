use twilight_model::channel::{embed::Embed, Message};

use crate::BotResult;

use super::{Pages, Pagination};

pub struct MatchComparePagination {
    msg: Message,
    pages: Pages,
    embeds: Vec<Embed>,
}

impl MatchComparePagination {
    pub fn new(msg: Message, embeds: Vec<Embed>) -> Self {
        Self {
            pages: Pages::new(1, embeds.len()),
            msg,
            embeds,
        }
    }
}

#[async_trait]
impl Pagination for MatchComparePagination {
    type PageData = Embed;

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
        Ok(self.embeds[self.page() - 1].clone())
    }
}
