use std::{collections::HashMap, fmt::Write};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::round, CowUtils, EmbedBuilder,
    FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::RecentListEntry,
    embeds::{ComboFormatter, KeyFormatter, PpFormatter},
    manager::{ redis::osu::CachedUser, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeCompletionFormatter, CachedUserExt,
    },
};

#[derive(PaginationBuilder)]
pub struct RecentListPagination {
    user: CachedUser,
    #[pagination(per_page = 10)]
    entries: Box<[RecentListEntry]>,
    maps: HashMap<u32, OsuMap, IntHasher>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for RecentListPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let mut description = String::with_capacity(512);

        for entry in entries {
            let RecentListEntry {
                idx,
                score,
                map_id,
                stars,
                max_pp,
                max_combo,
            } = entry;

            let map = self.maps.get(map_id).expect("missing map");

            let _ = write!(
                description,
                "**#{i} {grade}\t[{title} [{version}]]({OSU_BASE}b/{map_id})** [{stars:.2}★]",
                i = *idx + 1,
                grade = GradeCompletionFormatter::new(score, self.user.mode, map.n_objects()),
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = map.map_id(),
            );

            if score.mode == GameMode::Mania {
                let _ = write!(
                    description,
                    "\t{}",
                    KeyFormatter::new(&score.mods, map.attributes().build().cs as f32)
                );
            }

            description.push('\n');

            let _ = writeln!(
                description,
                "{pp}\t[ {combo} ]\t({acc}%)\t{ago}",
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                acc = round(score.accuracy),
                ago = HowLongAgoDynamic::new(&score.ended_at)
            );
        }

        if description.is_empty() {
            "No recent scores found".clone_into(&mut description);
        }

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url.as_ref())
            .title("List of recent scores:");

        BuildPage::new(embed, false)
            .content(self.content.clone())
            .boxed()
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
