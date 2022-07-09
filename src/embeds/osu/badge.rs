use std::fmt::Write;

use command_macros::EmbedData;

use crate::{
    custom_client::{OsekaiBadge, OsekaiBadgeOwner},
    embeds::{attachment, EmbedFields},
    pagination::Pages,
    util::{builder::FooterBuilder, constants::OSU_BASE, datetime::DATE_FORMAT, CowUtils},
};

#[derive(EmbedData)]
pub struct BadgeEmbed {
    fields: EmbedFields,
    footer: FooterBuilder,
    image: String,
    thumbnail: String,
    title: String,
    url: String,
}

impl BadgeEmbed {
    pub fn new(badge: &OsekaiBadge, owners: &[OsekaiBadgeOwner], pages: &Pages) -> Self {
        let mut owners_str = String::with_capacity(50 * owners.len().min(10));

        for owner in owners.iter().take(10) {
            let _ = writeln!(
                owners_str,
                ":flag_{code}: [{name}]({OSU_BASE}u/{user_id})",
                code = owner.country_code.to_ascii_lowercase(),
                name = owner.username.cow_escape_markdown(),
                user_id = owner.user_id
            );
        }

        if owners.len() > 10 {
            let _ = write!(owners_str, "and {} more...", owners.len() - 10);
        }

        let awarded_at = badge.awarded_at.format(DATE_FORMAT).unwrap();

        let fields = vec![
            field!("Owners", owners_str, false),
            field!("Awarded at", awarded_at, true),
            field!("Name", badge.name.clone(), true),
        ];

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages} • Check out osekai.net for more info");

        Self {
            fields,
            footer: FooterBuilder::new(footer_text),
            image: attachment("badge_owners.png"),
            thumbnail: badge.image_url.clone(),
            title: badge.description.clone(),
            url: format!("https://osekai.net/badges/?badge={}", badge.badge_id),
        }
    }
}
