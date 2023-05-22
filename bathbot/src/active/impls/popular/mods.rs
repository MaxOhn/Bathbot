use std::{fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsuTrackerModsEntry;
use bathbot_util::{numbers::WithComma, AuthorBuilder, EmbedBuilder, FooterBuilder};
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
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct PopularModsPagination {
    #[pagination(per_page = 20)]
    entries: Box<[OsuTrackerModsEntry]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for PopularModsPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page())];

        let author =
            AuthorBuilder::new("Most common mods in top plays").url("https://osutracker.com/stats");

        let idx = pages.index() + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            sizes.idx_left = sizes.idx_left.max(i.to_string().len());
            sizes.mods_left = sizes.mods_left.max(entry.mods.to_string().len());

            sizes.count_left = sizes
                .count_left
                .max(WithComma::new(entry.count).to_string().len());
        }

        for (entry, i) in entries.iter().skip(10).zip(idx + 10..) {
            sizes.idx_right = sizes.idx_right.max(i.to_string().len());
            sizes.mods_right = sizes.mods_right.max(entry.mods.to_string().len());

            sizes.count_right = sizes
                .count_right
                .max(WithComma::new(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 30);

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            // necessary to not mess up formatting
            #[allow(clippy::to_string_in_format_args)]
            let _ = write!(
                description,
                "`{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                i_len = sizes.idx_left,
                mods = entry.mods,
                pad = " ".repeat(sizes.mods_left - entry.mods.to_string().len()),
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count_left,
            );

            if let Some(entry) = entries.get(i + 10 - idx) {
                // necessary to not mess up formatting
                #[allow(clippy::to_string_in_format_args)]
                let _ = write!(
                    description,
                    " | `{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                    i = i + 10,
                    i_len = sizes.idx_right,
                    mods = entry.mods,
                    pad = " ".repeat(sizes.mods_right - entry.mods.to_string().len()),
                    count = WithComma::new(entry.count).to_string(),
                    c_len = sizes.count_right,
                );
            }

            description.push('\n');
        }

        description.pop();

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer);

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

#[derive(Default)]
struct Sizes {
    idx_left: usize,
    mods_left: usize,
    count_left: usize,
    idx_right: usize,
    mods_right: usize,
    count_right: usize,
}
