use twilight_model::channel::Message;

use crate::{embeds::MedalsListEmbed, BotResult};

use super::{Pages, Pagination};

pub struct MedalsListPagination {
    msg: Message,
    pages: Pages,
}

impl MedalsListPagination {
    pub fn new(msg: Message) -> Self {
        Self {
            pages: Pages::new(10, 0), // TODO
            msg,
        }
    }
}

#[async_trait]
impl Pagination for MedalsListPagination {
    type PageData = MedalsListEmbed;

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
        todo!()
    }
}
