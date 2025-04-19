use std::{
    collections::{BTreeMap, btree_map::Entry},
    fmt::Write,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{OsekaiBadge, OsekaiBadgeOwner};
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, constants::OSU_BASE, datetime::DATE_FORMAT, fields,
};
use eyre::{Result, WrapErr};
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
    embeds::attachment,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct BadgesPagination {
    #[pagination(per_page = 1)]
    badges: Box<[OsekaiBadge]>,
    owners: BTreeMap<usize, Box<[OsekaiBadgeOwner]>>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for BadgesPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let badge = &self.badges[idx];

        let owners = match self.owners.entry(idx) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let owners = Context::client()
                    .get_osekai_badge_owners(badge.badge_id)
                    .await
                    .wrap_err("Failed to get osekai badge owners")?;

                e.insert(owners.into_boxed_slice())
            }
        };

        let mut owners_str = String::with_capacity(50 * owners.len().min(10));

        for owner in owners.iter().take(10) {
            let _ = if owner.username.is_empty() {
                writeln!(
                    owners_str,
                    ":pirate_flag: [<user {user_id}>]({OSU_BASE}u/{user_id})",
                    user_id = owner.user_id
                )
            } else {
                writeln!(
                    owners_str,
                    ":flag_{code}: [{name}]({OSU_BASE}u/{user_id})",
                    code = owner.country_code.to_ascii_lowercase(),
                    name = owner.username.cow_escape_markdown(),
                    user_id = owner.user_id
                )
            };
        }

        if owners.len() > 10 {
            let _ = write!(owners_str, "and {} more...", owners.len() - 10);
        }

        let awarded_at = badge.awarded_at.format(DATE_FORMAT).unwrap();

        let fields = fields![
            "Owners", owners_str, false;
            "Awarded at", awarded_at, true;
            "Name", badge.name.to_string(), true;
        ];

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages} • Check out osekai.net for more info");

        let url = format!("https://osekai.net/badges/?badge={}", badge.badge_id);

        let embed = EmbedBuilder::new()
            .fields(fields)
            .footer(FooterBuilder::new(footer_text))
            .image(attachment("badge_owners.png"))
            .thumbnail(badge.image_url.as_ref())
            .title(badge.description.as_ref())
            .url(url);

        Ok(BuildPage::new(embed, true))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        handle_pagination_component(component, self.msg_owner, true, &mut self.pages).await
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        handle_pagination_modal(modal, self.msg_owner, true, &mut self.pages).await
    }
}
