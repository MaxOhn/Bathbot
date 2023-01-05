use std::collections::{btree_map::Entry, BTreeMap};

use bathbot_macros::pagination;
use bathbot_model::{OsekaiBadge, OsekaiBadgeOwner};
use eyre::{Result, WrapErr};
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    embeds::{BadgeEmbed, EmbedData},
};

use super::Pages;

#[pagination(per_page = 1, entries = "badges")]
pub struct BadgePagination {
    badges: Vec<OsekaiBadge>,
    owners: BTreeMap<usize, Vec<OsekaiBadgeOwner>>,
}

impl BadgePagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let idx = pages.index;
        let badge = &self.badges[idx];

        let owners = match self.owners.entry(idx) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let owners = ctx
                    .client()
                    .get_osekai_badge_owners(badge.badge_id)
                    .await
                    .wrap_err("failed to get osekai badge owners")?;

                e.insert(owners)
            }
        };

        Ok(BadgeEmbed::new(badge, owners, pages).build())
    }
}
