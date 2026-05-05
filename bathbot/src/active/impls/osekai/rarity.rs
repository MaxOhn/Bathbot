use std::{cmp, fmt::Write};

use bathbot_cache::model::CachedArchive;
use bathbot_macros::PaginationBuilder;
use bathbot_model::{ArchivedOsekaiRarityEntry, OsekaiMedal};
use bathbot_util::{EmbedBuilder, FooterBuilder, numbers::WithComma};
use eyre::Result;
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
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let limit = self.ranking.len().min(idx + pages.per_page());
        let ranking = &self.ranking[idx..limit];

        let mut description = String::with_capacity(1024);

        let iter = || ranking.iter().zip(pages.index() + 1..);

        let mut len_i = 0;
        let mut len_medal = 0;
        let mut len_count = 0;
        let mut len_rarity = 0;

        let mut buf = zmij::Buffer::new();

        fn fmt_count(entry: &ArchivedOsekaiRarityEntry) -> WithComma<u32> {
            WithComma::new(entry.count_achieved_by.to_native())
        }

        fn fmt_rarity<'buf>(
            entry: &ArchivedOsekaiRarityEntry,
            buf: &'buf mut zmij::Buffer,
        ) -> &'buf str {
            const DECIMALS: usize = 6;

            let mut rarity = buf.format(entry.frequency.to_native());

            if let Some(dot) = rarity.find('.')
                && dot + DECIMALS < rarity.len()
            {
                rarity = &rarity[..dot + 1 + DECIMALS];
            }

            rarity
        }

        for (entry, i) in iter() {
            len_i = cmp::max(len_i, i.to_string().len());
            len_medal = cmp::max(len_medal, entry.medal_name.as_ref().len());
            len_count = cmp::max(len_count, fmt_count(entry).to_string().len());
            len_rarity = cmp::max(len_rarity, fmt_rarity(entry, &mut buf).len());
        }

        for (entry, i) in iter() {
            let medal_name = entry.medal_name.as_ref();

            let _ = writeln!(
                description,
                "**`#{i:<len_i$}` [`{medal:<len_medal$}`]({url} \"{description}\")** `{count:>len_count$}` `{rarity:>len_rarity$}%`",
                medal = entry.medal_name,
                url = OsekaiMedal::name_to_url(medal_name),
                description = entry.description,
                count = fmt_count(entry).to_string(),
                rarity = fmt_rarity(entry, &mut buf),
            );
        }

        let title = "Medal Ranking based on amount of owners";
        let url = "https://inex.osekai.net/rankings/medals_rarity";

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!("Page {page}/{pages} • Check out osekai.net for more info");

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
