use std::{fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{CountryName, SnipeCountryPlayer};
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, osu::flag_url, CowUtils, EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::CountryCode;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::SnipeCountryListOrder,
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct SnipeCountryListPagination {
    #[pagination(per_page = 10)]
    players: Box<[(usize, SnipeCountryPlayer)]>,
    country: Option<(CountryName, CountryCode)>,
    order: SnipeCountryListOrder,
    author_idx: Option<usize>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SnipeCountryListPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let players = self
            .players
            .iter()
            .skip(self.pages.index())
            .take(self.pages.per_page());

        let order_text = match self.order {
            SnipeCountryListOrder::Count => "#1 count",
            SnipeCountryListOrder::Pp => "average pp of #1s",
            SnipeCountryListOrder::Stars => "average stars of #1s",
            SnipeCountryListOrder::WeightedPp => "weighted pp from #1s",
        };

        let (title, thumbnail) = match self.country.as_ref() {
            Some((country, code)) => {
                let title = format!(
                    "{country}{} #1 list, sorted by {order_text}",
                    if country.ends_with('s') { "'" } else { "'s" },
                );

                let thumbnail = flag_url(code.as_str());

                (title, thumbnail)
            }
            None => (
                format!("Global #1 statistics, sorted by {order_text}"),
                String::new(),
            ),
        };

        let mut description = String::with_capacity(512);

        for (idx, player) in players {
            let _ = writeln!(
                description,
                "**{idx}. [{name}]({OSU_BASE}users/{id})**: {w}Weighted pp: {weighted}{w}\n\
                    {c}Count: {count}{c} ~ {p}Avg pp: {pp}{p} ~ {s}Avg stars: {stars:.2}★{s}",
                name = player.username.cow_escape_markdown(),
                id = player.user_id,
                c = if self.order == SnipeCountryListOrder::Count {
                    "__"
                } else {
                    ""
                },
                p = if self.order == SnipeCountryListOrder::Pp {
                    "__"
                } else {
                    ""
                },
                s = if self.order == SnipeCountryListOrder::Stars {
                    "__"
                } else {
                    ""
                },
                w = if self.order == SnipeCountryListOrder::WeightedPp {
                    "__"
                } else {
                    ""
                },
                count = WithComma::new(player.count_first),
                pp = WithComma::new(player.avg_pp),
                stars = player.avg_sr,
                weighted = WithComma::new(player.pp),
            );
        }

        description.pop();

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();
        let mut footer_text = format!("Page {page}/{pages}");

        if let Some(idx) = self.author_idx {
            let _ = write!(footer_text, " ~ Your position: {}", idx + 1);
        }

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(thumbnail)
            .title(title);

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(ctx, component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, false, &mut self.pages)
    }
}
