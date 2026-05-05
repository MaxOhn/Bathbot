use std::{cmp, collections::BTreeMap, fmt::Write, num::NonZeroU8};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{OsekaiMedal, OsekaiUserEntry};
use bathbot_util::{
    EmbedBuilder, FooterBuilder, constants::OSU_BASE, datetime::NAIVE_DATETIME_FORMAT,
    numbers::round,
};
use eyre::{Context as _, Result};
use rkyv::{
    Deserialize,
    rancor::{Panic, ResultExt, Strategy},
};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalCountPagination {
    #[pagination(per_page = 10, len = "total")]
    ranking: BTreeMap<usize, OsekaiUserEntry>,
    total: usize,
    country: Option<String>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalCountPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;

        let count = self
            .ranking
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() && count < self.total - pages.index() {
            let offset = pages.index() / 50;
            let page = NonZeroU8::new((offset + 1) as u8).unwrap();

            let ranking = Context::redis()
                .osekai_medal_count(self.country.as_deref(), page)
                .await
                .wrap_err("Failed to get more ranking entries")?;

            let iter = ranking.data.iter().enumerate().map(|(i, entry)| {
                let entry: OsekaiUserEntry = entry
                    .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                    .always_ok();

                (offset * 50 + i, entry)
            });

            self.ranking.extend(iter);
        }

        let mut description = String::with_capacity(1024);

        let idx = pages.index();
        let limit = idx + pages.per_page();

        let iter = || {
            self.ranking
                .range(idx..limit)
                .map(|(i, entry)| (i + 1, entry))
        };

        let mut len_i = 0;
        let mut len_name = 0;
        let mut len_count = 0;
        let mut len_percent = 0;

        let mut buf = zmij::Buffer::new();

        fn fmt_percent<'buf>(entry: &OsekaiUserEntry, buf: &'buf mut zmij::Buffer) -> &'buf str {
            buf.format(round(entry.medal_percentage))
        }

        for (i, entry) in iter() {
            len_i = cmp::max(len_i, i.to_string().len());
            len_name = cmp::max(len_name, entry.username.len());
            len_count = cmp::max(len_count, entry.count_medals.to_string().len());
            len_percent = cmp::max(len_percent, fmt_percent(entry, &mut buf).len());
        }

        for (i, entry) in iter() {
            let medal_name = entry.rarest_medal.name.as_ref();
            let medal_url = OsekaiMedal::name_to_url(medal_name);

            let _ = writeln!(
                description,
                "**`#{i:<len_i$}`** :flag_{country}: [**`{user:<len_name$}`**]({OSU_BASE}u/{user_id}) \
                `{count:>len_count$}` `{percent:>len_percent$}%` ▸ [{medal}]({medal_url} \"Achieved {achieved_datetime}\nTotal owners: {total_owners} ({total_frequency}%)\")",
                country = entry.country_code.to_ascii_lowercase(),
                user = entry.username,
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
