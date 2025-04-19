use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_model::{OsekaiMedal, OsekaiUserEntry};
use bathbot_util::{CowUtils, EmbedBuilder, FooterBuilder, constants::OSU_BASE, numbers::round};
use eyre::Result;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalCountPagination {
    #[pagination(per_page = 10)]
    ranking: Box<[OsekaiUserEntry]>,
    author_idx: Option<usize>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalCountPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let limit = self.ranking.len().min(idx + pages.per_page());
        let ranking = &self.ranking[idx..limit];

        let mut description = String::with_capacity(1024);

        for (entry, idx) in ranking.iter().zip(pages.index()..) {
            let medal_name = entry.rarest_medal.as_ref();

            let medal_url = match OsekaiMedal::name_to_url(medal_name) {
                Ok(url) => url,
                Err(err) => {
                    warn!(?err);

                    OsekaiMedal::backup_name_to_url(medal_name)
                }
            };

            let _ = writeln!(
                description,
                "**{i}.** :flag_{country}: [{author}**{user}**{author}]({OSU_BASE}u/{user_id}): \
                `{count}` (`{percent}%`) ▸ [{medal}]({medal_url})",
                i = idx + 1,
                country = entry.country_code.to_ascii_lowercase(),
                author = if self.author_idx == Some(idx) {
                    "__"
                } else {
                    ""
                },
                user = entry.username.cow_escape_markdown(),
                user_id = entry.user_id,
                count = entry.medal_count,
                percent = round(entry.completion),
                medal = entry.rarest_medal,
            );
        }

        let title = "User Ranking based on amount of owned medals";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

        let page = pages.curr_page();
        let pages = pages.last_page();
        let mut footer_text = format!("Page {page}/{pages} • ");

        if let Some(idx) = self.author_idx {
            let _ = write!(footer_text, "Your position: {} • ", idx + 1);
        }

        footer_text.push_str("Check out osekai.net for more info");

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .title(title)
            .url(url);

        Ok(BuildPage::new(embed, false))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages).await
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages).await
    }
}
