use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use serenity::async_trait;

pub struct CommandCountPagination {
    pages: Pages,
    booted_up: String,
    cmd_counts: Vec<(String, u32)>,
}

impl CommandCountPagination {
    pub fn new(cmd_counts: Vec<(String, u32)>, booted_up: String) -> Self {
        Self {
            pages: Pages::new(15, cmd_counts.len()),
            cmd_counts,
            booted_up,
        }
    }
}

#[async_trait]
impl Pagination for CommandCountPagination {
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<BasicEmbedData, Error> {
        let sub_list: Vec<(&String, u32)> = self
            .cmd_counts
            .iter()
            .skip(self.index())
            .take(self.per_page())
            .map(|(name, amount)| (name, *amount))
            .collect();
        Ok(BasicEmbedData::create_command_counter(
            sub_list,
            &self.booted_up,
            self.index() + 1,
            (self.page(), self.total_pages()),
        ))
    }
}
