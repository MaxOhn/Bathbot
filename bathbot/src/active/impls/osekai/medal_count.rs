use std::{cmp, fmt::Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{OsekaiMedal, OsekaiUserEntry};
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, constants::OSU_BASE, datetime::NAIVE_DATETIME_FORMAT,
    numbers::round,
};
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

        let iter = || ranking.iter().zip(pages.index() + 1..);

        let mut len_i = 0;
        let mut len_name = 0;
        let mut len_count = 0;
        let mut len_percent = 0;

        let mut buf = zmij::Buffer::new();

        fn fmt_percent<'buf>(entry: &OsekaiUserEntry, buf: &'buf mut zmij::Buffer) -> &'buf str {
            buf.format(round(entry.medal_percentage))
        }

        for (entry, i) in iter() {
            len_i = cmp::max(len_i, i.to_string().len());
            len_name = cmp::max(len_name, entry.username.len());
            len_count = cmp::max(len_count, entry.count_medals.to_string().len());
            len_percent = cmp::max(len_percent, fmt_percent(entry, &mut buf).len());
        }

        for (entry, i) in iter() {
            let medal_name = entry.rarest_medal.name.as_ref();
            let medal_url = OsekaiMedal::name_to_url(medal_name);

            let _ = writeln!(
                description,
                "**`#{i:<len_i$}`** :flag_{country}: [{author}**`{user:<len_name$}`**{author}]({OSU_BASE}u/{user_id}) \
                `{count:>len_count$}` `{percent:>len_percent$}%` ▸ [{medal}]({medal_url} \"Achieved {achieved_datetime}\nTotal owners: {total_owners} ({total_frequency}%)\")",
                country = entry.country_code.to_ascii_lowercase(),
                author = if self.author_idx == Some(i) { "__" } else { "" },
                user = entry.username.cow_escape_markdown(),
                user_id = entry.user_id,
                count = entry.count_medals,
                percent = fmt_percent(entry, &mut buf),
                medal = entry.rarest_medal.name,
                achieved_datetime = entry
                    .rarest_medal_achieved
                    .format(NAIVE_DATETIME_FORMAT)
                    .unwrap(),
                total_owners = entry.rarest_medal.count_achieved_by,
                total_frequency = entry.rarest_medal.frequency,
            );
        }

        let title = "User Ranking based on amount of owned medals";
        let url = "https://inex.osekai.net/rankings/medals_users";

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
