use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{CowUtils, EmbedBuilder, FooterBuilder, attachment};
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
    commands::osu::{MedalEntryCommon, MedalsCommonUser},
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
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let medals = &self.medals[idx..self.medals.len().min(idx + pages.per_page())];

        let mut description = String::with_capacity(512);

        for (entry, i) in medals.iter().zip(pages.index() + 1..) {
            let url = match entry.medal.url() {
                Ok(url) => url,
                Err(err) => {
                    warn!(?err);

                    entry.medal.backup_url()
                }
            };

            let url = url.cow_replace("%25", "%");

            let _ = writeln!(
                description,
                "**#{i} [{name}]({url})**",
                name = entry.medal.name,
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
