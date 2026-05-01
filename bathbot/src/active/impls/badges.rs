use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsekaiBadge;
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, attachment, constants::OSU_BASE, datetime::DATE_FORMAT,
    fields,
};
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
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct BadgesPagination {
    #[pagination(per_page = 1)]
    badges: Box<[OsekaiBadge]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for BadgesPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let badge = &self.badges[idx];

        let users = &badge.users;

        let mut owners_str = String::with_capacity(50 * users.len().min(10));

        for user in users.iter().take(10) {
            let _ = if user.username.is_empty() {
                writeln!(
                    owners_str,
                    ":pirate_flag: [<user {user_id}>]({OSU_BASE}u/{user_id})",
                    user_id = user.user_id
                )
            } else {
                writeln!(
                    owners_str,
                    "{flag} [{name}]({OSU_BASE}u/{user_id})",
                    flag = if let Some(code) = user.country_code.as_deref() {
                        format!(":flag_{}:", code.to_ascii_lowercase())
                    } else {
                        String::new()
                    },
                    name = user.username.cow_escape_markdown(),
                    user_id = user.user_id
                )
            };
        }

        if users.len() > 10 {
            let _ = write!(owners_str, "and {} more...", users.len() - 10);
        }

        let awarded_at = badge.first_date_awarded.format(DATE_FORMAT).unwrap();

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
