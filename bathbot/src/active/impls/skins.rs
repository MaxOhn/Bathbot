use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_psql::model::configs::SkinEntry;
use bathbot_util::{EmbedBuilder, FooterBuilder, constants::OSU_BASE};
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

// TODO: replace with ranking pagination once it supports code block hyperlinks
#[derive(PaginationBuilder)]
pub struct SkinsPagination {
    #[pagination(per_page = 12)]
    entries: Box<[SkinEntry]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SkinsPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        const PER_PAGE: usize = 12;
        const PER_SIDE: usize = PER_PAGE / 2;

        let Self { pages, entries, .. } = &*self;

        let idx = pages.index();
        let end_left = entries.len().min(idx + PER_SIDE);

        let left = &entries[idx..end_left];

        let right = if entries.len() > idx + PER_SIDE {
            let end_right = entries.len().min(idx + PER_PAGE);

            &entries[idx + PER_SIDE..end_right]
        } else {
            &[]
        };

        let left_lengths = Lengths::new(idx, left);
        let right_lengths = Lengths::new(idx + PER_SIDE, right);

        // Ensuring the right side has ten elements for the zip
        let user_iter = left.iter().zip((0..PER_SIDE).map(|i| right.get(i)));

        let mut description = String::with_capacity(2048);

        for ((left, right), idx) in user_iter.zip(idx + 1..) {
            let _ = write!(
                description,
                "`#{idx:<idx_len$}` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) [`Skin`]({skin_url} \"{skin_tooltip}\")",
                idx_len = left_lengths.idx,
                name = left.username,
                name_len = left_lengths.name,
                user_id = left.user_id,
                skin_url = left.skin_url,
                skin_tooltip = left.skin_url.trim_start_matches("https://"),
            );

            if let Some(right) = right {
                let _ = write!(
                    description,
                    "|`#{idx:<idx_len$}` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) [`Skin`]({skin_url} \"{skin_tooltip}\")",
                    idx = idx + PER_SIDE,
                    idx_len = right_lengths.idx,
                    name = right.username,
                    name_len = right_lengths.name,
                    user_id = right.user_id,
                    skin_url = right.skin_url,
                    skin_tooltip = right.skin_url.trim_start_matches("https://"),
                );
            }

            description.push('\n');
        }

        let title = "All linked skins:".to_owned();

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer = FooterBuilder::new(format!("Page {page}/{pages}"));

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(footer)
            .title(title);

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

struct Lengths {
    idx: usize,
    name: usize,
}

impl Lengths {
    fn new(start: usize, iter: &[SkinEntry]) -> Self {
        let mut idx_len = 0;
        let mut name_len = 0;

        for (entry, i) in iter.iter().zip(start + 1..) {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);
            name_len = name_len.max(entry.username.len());
        }

        Lengths {
            idx: idx_len,
            name: name_len,
        }
    }
}
