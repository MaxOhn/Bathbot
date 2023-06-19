use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsuTrackerCountryScore;
use bathbot_util::{
    constants::OSU_BASE,
    numbers::{round, WithComma},
    osu::flag_url,
    CowUtils, EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{OsuTrackerCountryDetailsCompact, ScoreOrder},
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct CountryTopPagination {
    details: OsuTrackerCountryDetailsCompact,
    #[pagination(per_page = 10)]
    scores: Box<[(OsuTrackerCountryScore, usize)]>,
    sort_by: ScoreOrder,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for CountryTopPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let idx = self.pages.index();
        let scores = &self.scores[idx..self.scores.len().min(idx + self.pages.per_page())];

        let national = !self.details.code.is_empty();

        let url = format!(
            "https://osutracker.com/country/{code}",
            code = national
                .then(|| self.details.code.as_str())
                .unwrap_or("Global")
        );

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

        let title = format!("Total PP: {}pp", WithComma::new(self.details.pp));

        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter() {
            let _ = writeln!(
                description,
                "**#{i}** [{map_name}]({OSU_BASE}b/{map_id}) **+{mods}**\n\
                by __[{user}]({OSU_BASE}u/{adjusted_user})__ • **{pp}pp** • {acc}% • <t:{timestamp}:R>{appendix}",
                map_name = score.name.cow_escape_markdown(),
                map_id = score.map_id,
                mods = score.mods,
                user = score.player.cow_escape_markdown(),
                adjusted_user = score.player.cow_replace(' ', "%20"),
                pp = round(score.pp),
                acc = round(score.acc),
                timestamp = score.ended_at.unix_timestamp(),
                appendix = OrderAppendix::new(self.sort_by, score),
            );
        }

        let thumbnail = national
            .then(|| flag_url(self.details.code.as_str()))
            .unwrap_or_default();

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(footer)
            .thumbnail(thumbnail)
            .title(title)
            .url(url);

        BuildPage::new(embed, false)
            .content(self.content.clone())
            .boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
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

struct OrderAppendix<'s> {
    sort_by: ScoreOrder,
    score: &'s OsuTrackerCountryScore,
}

impl<'s> OrderAppendix<'s> {
    pub fn new(sort_by: ScoreOrder, score: &'s OsuTrackerCountryScore) -> Self {
        Self { sort_by, score }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            ScoreOrder::Acc | ScoreOrder::Date | ScoreOrder::Pp => Ok(()),
            ScoreOrder::Length => {
                let clock_rate = self.score.mods.legacy_clock_rate();
                let secs = (self.score.seconds_total as f32 / clock_rate) as u32;

                write!(f, " • `{}:{:0>2}`", secs / 60, secs % 60)
            }
            ScoreOrder::Misses => write!(
                f,
                " • {}miss{plural}",
                self.score.n_misses,
                plural = if self.score.n_misses != 1 { "es" } else { "" }
            ),
            _ => unreachable!(),
        }
    }
}
