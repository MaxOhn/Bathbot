use twilight_model::channel::Message;

use crate::{util::builder::EmbedBuilder, BotResult};

use super::{Pages, Pagination};

pub struct MatchComparePagination {
    msg: Message,
    pages: Pages,
    embeds: Vec<EmbedBuilder>,
}

impl MatchComparePagination {
    pub fn new(msg: Message, embeds: Vec<EmbedBuilder>) -> Self {
        Self {
            pages: Pages::new(1, embeds.len()),
            msg,
            embeds,
        }
    }
}

#[async_trait]
impl Pagination for MatchComparePagination {
    type PageData = EmbedBuilder;

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
