use command_macros::pagination;
use rosu_v2::model::user::User;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::MedalType,
    embeds::{EmbedData, MedalsMissingEmbed},
};

use super::Pages;

#[pagination(per_page = 15, entries = "medals")]
pub struct MedalsMissingPagination {
    user: User,
    medals: Vec<MedalType>,
    medal_count: (usize, usize),
}

impl MedalsMissingPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let limit = self.medals.len().min(idx + pages.per_page);

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
