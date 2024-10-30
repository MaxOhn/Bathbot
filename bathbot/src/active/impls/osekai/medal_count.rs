use std::fmt::Write;

use bathbot_cache::value::CachedArchive;
use bathbot_macros::PaginationBuilder;
use bathbot_model::{ArchivedOsekaiUserEntry, OsekaiMedal};
use bathbot_util::{constants::OSU_BASE, numbers::round, CowUtils, EmbedBuilder, FooterBuilder};
use eyre::Result;
use futures::future::BoxFuture;
use rkyv::vec::ArchivedVec;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalCountPagination {
    #[pagination(per_page = 10)]
    ranking: CachedArchive<ArchivedVec<ArchivedOsekaiUserEntry>>,
    author_idx: Option<usize>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalCountPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
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

            let _ =
                writeln!(
                description,
                "**{i}.** :flag_{country}: [{author}**{user}**{author}]({OSU_BASE}u/{user_id}): \
                `{count}` (`{percent}%`) ▸ [{medal}]({medal_url})",
                i = idx + 1,
                country = entry.country_code.to_ascii_lowercase(),
                author = if self.author_idx == Some(idx) { "__" } else { "" },
                user = entry.username.cow_escape_markdown(),
                user_id = entry.user_id,
                count = entry.medal_count,
                percent = round(entry.completion.to_native()),
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

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}
