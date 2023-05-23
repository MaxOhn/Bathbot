use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{CowUtils, EmbedBuilder, FooterBuilder};
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
    commands::osu::{MedalEntryCommon, MedalsCommonUser},
    core::Context,
    embeds::attachment,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsCommonPagination {
    user1: MedalsCommonUser,
    user2: MedalsCommonUser,
    #[pagination(per_page = 10)]
    medals: Box<[MedalEntryCommon]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsCommonPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let medals = &self.medals[idx..self.medals.len().min(idx + pages.per_page())];

        let mut description = String::with_capacity(512);

        for (entry, i) in medals.iter().zip(pages.index() + 1..) {
            let _ = writeln!(
                description,
                "**#{i} [{name}](https://osekai.net/medals/?medal={medal})**",
                name = entry.medal.name,
                medal = entry
                    .medal
                    .name
                    .cow_replace(' ', "+")
                    .cow_replace(',', "%2C"),
            );

            let (timestamp1, timestamp2, first_earlier) = match (entry.achieved1, entry.achieved2) {
                (Some(a1), Some(a2)) => (
                    Some(a1.unix_timestamp()),
                    Some(a2.unix_timestamp()),
                    a1 < a2,
                ),
                (Some(a1), None) => (Some(a1.unix_timestamp()), None, true),
                (None, Some(a2)) => (None, Some(a2.unix_timestamp()), false),
                (None, None) => unreachable!(),
            };

            let _ = writeln!(
                description,
                ":{medal1}_place: `{name1}`: {timestamp1} \
                :{medal2}_place: `{name2}`: {timestamp2}",
                medal1 = if first_earlier { "first" } else { "second" },
                name1 = self.user1.name,
                timestamp1 = TimestampFormatter::new(timestamp1),
                medal2 = if first_earlier { "second" } else { "first" },
                name2 = self.user2.name,
                timestamp2 = TimestampFormatter::new(timestamp2),
            );
        }

        description.pop();

        let footer_text = format!(
            "🥇 count | {}: {} • {}: {}",
            self.user1.name, self.user1.winner, self.user2.name, self.user2.winner
        );

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(attachment("avatar_fuse.png"))
            .title("Who got which medal first");

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

struct TimestampFormatter {
    timestamp: Option<i64>,
}

impl TimestampFormatter {
    fn new(timestamp: Option<i64>) -> Self {
        Self { timestamp }
    }
}

impl Display for TimestampFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.timestamp {
            Some(timestamp) => write!(f, "<t:{timestamp}:d>"),
            None => f.write_str("Never"),
        }
    }
}
