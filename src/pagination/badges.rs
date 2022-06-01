use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
};

use command_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    custom_client::{OsekaiBadge, OsekaiBadgeOwner},
    embeds::{BadgeEmbed, EmbedData},
    BotResult,
};

use super::Pages;

#[pagination(per_page = 1, entries = "badges")]
pub struct BadgePagination {
    ctx: Arc<Context>,
    badges: Vec<OsekaiBadge>,
    owners: BTreeMap<usize, Vec<OsekaiBadgeOwner>>,
}

impl BadgePagination {
    pub async fn build_page(&mut self, pages: &Pages) -> BotResult<Embed> {
        let idx = pages.index;
        let badge = &self.badges[idx];

        let owners = match self.owners.entry(idx) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let owners = self
                    .ctx
                    .client()
                    .get_osekai_badge_owners(badge.badge_id)
                    .await?;

                e.insert(owners)
            }
        };

        Ok(BadgeEmbed::new(badge, owners, pages).build())
    }
}
