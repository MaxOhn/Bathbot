use std::{collections::HashMap, fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsuTrackerMapsetEntry;
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
    IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::MapsetEntry,
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct PopularMapsetsPagination {
    #[pagination(per_page = 10)]
    entries: Box<[OsuTrackerMapsetEntry]>,
    mapsets: HashMap<u32, MapsetEntry, IntHasher>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for PopularMapsetsPagination {
    fn build_page<'a>(&'a mut self, ctx: Arc<Context>) -> BoxFuture<'a, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(ctx, component, self.msg_owner, true, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, true, &mut self.pages)
    }
}

impl PopularMapsetsPagination {
    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let pages = &self.pages;
        let idx = pages.index();
        let entries = &self.entries[idx..self.entries.len().min(idx + pages.per_page())];

        for entry in entries {
            let mapset_id = entry.mapset_id;

            if self.mapsets.contains_key(&mapset_id) {
                continue;
            }

            let mapset = ctx.osu_map().mapset(mapset_id).await?;

            let entry = MapsetEntry {
                creator: mapset.creator.into(),
                name: format!("{} - {}", mapset.artist, mapset.title),
                mapset_id,
                ranked_date: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
                user_id: mapset.user_id as u32,
            };

            self.mapsets.insert(mapset_id, entry);
        }

        let author = AuthorBuilder::new("Most common mapsets in top plays")
            .url("https://osutracker.com/stats");

        let idx = pages.index() + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().zip(idx..) {
            sizes.idx = sizes.idx.max(i.to_string().len());

            sizes.count = sizes
                .count
                .max(WithComma::new(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 140);

        for (entry, i) in entries.iter().zip(idx..) {
            let mapset = self.mapsets.get(&entry.mapset_id).expect("missing mapset");

            // necessary to not mess up formatting
            #[allow(clippy::to_string_in_format_args)]
            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}s/{mapset_id})\n\
            ⯈ [{creator}]({OSU_BASE}u/{user_id}) • <t:{timestamp}:R>",
                i_len = sizes.idx,
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count,
                name = mapset.name.cow_escape_markdown(),
                mapset_id = entry.mapset_id,
                creator = mapset.creator.cow_escape_markdown(),
                user_id = mapset.user_id,
                timestamp = mapset.ranked_date.unix_timestamp(),
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

        Ok(BuildPage::new(embed, true))
    }
}

#[derive(Default)]
struct Sizes {
    idx: usize,
    count: usize,
}
