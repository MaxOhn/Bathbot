use std::fmt::Write;

use bathbot_cache::model::CachedArchive;
use bathbot_macros::PaginationBuilder;
use bathbot_model::{ArchivedOsekaiRarityEntry, OsekaiMedal};
use bathbot_util::{EmbedBuilder, FooterBuilder, numbers::round};
use eyre::Result;
use futures::future::BoxFuture;
use rkyv::vec::ArchivedVec;
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
pub struct MedalRarityPagination {
    #[pagination(per_page = 15)]
    ranking: CachedArchive<ArchivedVec<ArchivedOsekaiRarityEntry>>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalRarityPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let limit = self.ranking.len().min(idx + pages.per_page());
        let ranking = &self.ranking[idx..limit];

        let mut description = String::with_capacity(1024);

        for (entry, i) in ranking.iter().zip(pages.index() + 1..) {
            let medal_name = entry.medal_name.as_ref();

            let url = match OsekaiMedal::name_to_url(medal_name) {
                Ok(url) => url,
                Err(err) => {
                    warn!(?err);

                    OsekaiMedal::backup_name_to_url(medal_name)
                }
            };

            let _ = writeln!(
                description,
                "**#{i} [{medal}]({url} \"{description}\")**: `{rarity}%`",
                medal = entry.medal_name,
                rarity = round(entry.possession_percent.to_native()),
                description = entry.description,
            );
        }

        let title = "Medal Ranking based on rarity";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!("Page {page}/{pages} • Check out osekai.net for more info");

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
