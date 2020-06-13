use super::{create_collector, Pages, Pagination};

use crate::{embeds::CommandCounterEmbed, Error};

use serenity::{
    async_trait,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
};

pub struct CommandCountPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    booted_up: String,
    cmd_counts: Vec<(String, u32)>,
}

impl CommandCountPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        cmd_counts: Vec<(String, u32)>,
        booted_up: String,
    ) -> Self {
        let collector = create_collector(ctx, &msg, author, 60).await;
        Self {
            msg,
            collector,
            pages: Pages::new(15, cmd_counts.len()),
            cmd_counts,
            booted_up,
        }
    }
}

#[async_trait]
impl Pagination for CommandCountPagination {
    type PageData = CommandCounterEmbed;
    fn msg(&mut self) -> &mut Message {
        &mut self.msg
    }
    fn collector(&mut self) -> &mut ReactionCollector {
        &mut self.collector
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        let sub_list: Vec<(&String, u32)> = self
            .cmd_counts
            .iter()
            .skip(self.index())
            .take(self.per_page())
            .map(|(name, amount)| (name, *amount))
            .collect();
        Ok(CommandCounterEmbed::new(
            sub_list,
            &self.booted_up,
            self.index() + 1,
            (self.page(), self.total_pages()),
        ))
    }
}
