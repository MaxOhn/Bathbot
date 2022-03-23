use std::{collections::BTreeMap, sync::Arc};

use twilight_model::channel::Message;

use crate::{
    core::Context,
    custom_client::{OsekaiBadge, OsekaiBadgeOwner},
    embeds::BadgeEmbed,
    BotResult,
};

use super::{Pages, Pagination};

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

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let idx = self.pages.index;
        let badge = &self.badges[idx];

        if !self.owners.contains_key(&idx) {
            let owners = self
                .ctx
                .clients
                .custom
                .get_osekai_badge_owners(badge.badge_id)
                .await?;

            self.owners.insert(idx, owners);
        }

        let owners = &self.owners[&idx];
        let embed = BadgeEmbed::new(badge, owners, (idx + 1, self.pages.total_pages));

        Ok(embed)
    }
}
