use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
};

use command_macros::BasePagination;
use twilight_model::channel::Message;

use crate::{
    core::Context,
    custom_client::{OsekaiBadge, OsekaiBadgeOwner},
    embeds::BadgeEmbed,
    BotResult,
};

use super::{Pages, Pagination};

#[derive(BasePagination)]
#[pagination(single_step = 1)]
pub struct BadgePagination {
    msg: Message,
    pages: Pages,
    badges: Vec<OsekaiBadge>,
    owners: BTreeMap<usize, Vec<OsekaiBadgeOwner>>,
    ctx: Arc<Context>,
}

impl BadgePagination {
    pub fn new(
        msg: Message,
        badges: Vec<OsekaiBadge>,
        owners: BTreeMap<usize, Vec<OsekaiBadgeOwner>>,
        ctx: Arc<Context>,
    ) -> Self {
        Self {
            pages: Pages::new(1, badges.len()),
            msg,
            badges,
            owners,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for BadgePagination {
    type PageData = BadgeEmbed;

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let idx = self.pages.index;
        let badge = &self.badges[idx];

        let owners = match self.owners.entry(idx) {
            Entry::Occupied(e) => &*e.into_mut(),
            Entry::Vacant(e) => {
                let owners = self
                    .ctx
                    .client()
                    .get_osekai_badge_owners(badge.badge_id)
                    .await?;

                &*e.insert(owners)
            }
        };

        let embed = BadgeEmbed::new(badge, owners, (idx + 1, self.pages.total_pages));

        Ok(embed)
    }
}
