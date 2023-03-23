use bathbot_macros::pagination;
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::MedalType,
    embeds::{EmbedData, MedalsMissingEmbed},
    manager::redis::{osu::User, RedisData},
};

use super::Pages;

#[pagination(per_page = 15, entries = "medals")]
pub struct MedalsMissingPagination {
    user: RedisData<User>,
    medals: Vec<MedalType>,
    medal_count: (usize, usize),
}

impl MedalsMissingPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let limit = self.medals.len().min(idx + pages.per_page());

        let embed = MedalsMissingEmbed::new(
            &self.user,
            &self.medals[idx..limit],
            self.medal_count,
            limit == self.medals.len(),
            pages,
        );

        embed.build()
    }
}
