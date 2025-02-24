use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{EmbedBuilder, FooterBuilder, constants::OSU_BASE};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::tracking::TracklistUserEntry,
    util::{
        Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};

#[derive(PaginationBuilder)]
pub struct TrackListPagination {
    #[pagination(per_page = 15)]
    entries: Box<[TracklistUserEntry]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for TrackListPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let mut description = String::with_capacity(entries.len() * 100);

        for entry in entries {
            let TracklistUserEntry {
                name,
                user_id,
                mode,
                params,
            } = entry;

            let _ = writeln!(
                description,
                "[`{name}`]({OSU_BASE}u/{user_id}) {mode}: \
                `Index: {index}` • `PP: {pp}` • `Combo percent: {combo_percent}%`",
                mode = Emote::from(*mode),
                index = params.index(),
                pp = params.pp(),
                combo_percent = params.combo_percent(),
            );
        }

        if description.is_empty() {
            description.push_str("None");
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!(
            "Page {page}/{pages} • Total tracked: {}",
            self.entries.len()
        );

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .title("Tracked osu! users in this channel:");

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}
