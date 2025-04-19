use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{CountryName, SnipeCountryListOrder, SnipeCountryPlayer};
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder,
    constants::OSU_BASE,
    numbers::{WithComma, round},
    osu::flag_url,
};
use eyre::Result;
use rosu_v2::prelude::CountryCode;
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
    async fn build_page(&mut self) -> Result<BuildPage> {
        let players = self
            .players
            .iter()
            .skip(self.pages.index())
            .take(self.pages.per_page());

        let order_text = match self.order {
            SnipeCountryListOrder::Count => "#1 count",
            SnipeCountryListOrder::AvgPp => "average pp of #1s",
            SnipeCountryListOrder::AvgStars => "average stars of #1s",
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
                "**#{idx} [{name}]({OSU_BASE}users/{id})**: {w}Weighted pp: {weighted}{w}\n\
                {c}Count: {count}{c} {avg_pp}• {s}Avg stars: {stars:.2}★{s}",
                name = player.username.cow_escape_markdown(),
                id = player.user_id,
                c = if self.order == SnipeCountryListOrder::Count {
                    "__"
                } else {
                    ""
                },
                s = if self.order == SnipeCountryListOrder::AvgStars {
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
                avg_pp = AveragePpFormatter {
                    pp: player.avg_pp,
                    underline: self.order == SnipeCountryListOrder::AvgPp,
                },
                stars = player.avg_sr,
                weighted = WithComma::new(player.pp),
            );
        }

        description.pop();

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();
        let mut footer_text = format!("Page {page}/{pages}");

        if let Some(idx) = self.author_idx {
            let _ = write!(footer_text, " • Your position: {}", idx + 1);
        }

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(thumbnail)
            .title(title);

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

struct AveragePpFormatter {
    pp: Option<f32>,
    underline: bool,
}

impl Display for AveragePpFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let Some(pp) = self.pp else {
            return Ok(());
        };

        write!(
            f,
            "• {underline}Avg pp: {pp}{underline} ",
            pp = WithComma::new(round(pp)),
            underline = if self.underline { "__" } else { "" }
        )
    }
}
