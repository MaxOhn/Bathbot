use std::{fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsuTrackerPpEntry;
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
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
pub struct PopularMapsPagination {
    pp: u32,
    #[pagination(per_page = 10)]
    entries: Box<[OsuTrackerPpEntry]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for PopularMapsPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page())];

        let author_text = format!(
            "Most common maps in top plays: {}-{}pp",
            self.pp,
            self.pp + 100
        );
        let author = AuthorBuilder::new(author_text).url("https://osutracker.com/stats");

        let idx = pages.index() + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().zip(idx..) {
            sizes.idx = sizes.idx.max(i.to_string().len());

            sizes.count = sizes
                .count
                .max(WithComma::new(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 100);

        for (entry, i) in entries.iter().zip(idx..) {
            // necessary to not mess up formatting
            #[allow(clippy::to_string_in_format_args)]
            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}b/{map_id})",
                i_len = sizes.idx,
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count,
                name = entry.name.cow_escape_markdown(),
                map_id = entry.map_id,
            );
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
        ctx: Arc<Context>,
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
    idx: usize,
    count: usize,
}
