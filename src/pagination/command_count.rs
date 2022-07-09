use command_macros::pagination;
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::embeds::{CommandCounterEmbed, EmbedData};

use super::Pages;

#[pagination(per_page = 15, entries = "cmd_counts")]
pub struct CommandCountPagination {
    booted_up: OffsetDateTime,
    cmd_counts: Vec<(String, u32)>,
}

impl CommandCountPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let sub_list: Vec<(&String, u32)> = self
            .cmd_counts
            .iter()
            .skip(pages.index)
            .take(pages.per_page)
            .map(|(name, amount)| (name, *amount))
            .collect();

        CommandCounterEmbed::new(sub_list, &self.booted_up, pages).build()
    }
}
