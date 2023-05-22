use std::{cmp::Ordering, collections::HashMap, fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{constants::OSU_BASE, EmbedBuilder, FooterBuilder, IntHasher};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::Username;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{CommonScore, CompareTopMap},
    core::Context,
    embeds::attachment,
    util::interaction::{InteractionComponent, InteractionModal},
};

type CachedMaps = HashMap<u32, ([CommonScore; 2], CompareTopMap), IntHasher>;

#[derive(PaginationBuilder)]
pub struct CompareTopPagination {
    name1: Username,
    name2: Username,
    #[pagination(per_page = 10)]
    maps: CachedMaps,
    map_pps: Box<[(u32, f32)]>,
    wins: [u8; 2],
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for CompareTopPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let map_pps = &self.map_pps[idx..(idx + pages.per_page()).min(self.maps.len())];

        let mut description = String::with_capacity(1024);

        for ((map_id, _), i) in map_pps.iter().zip(pages.index() + 1..) {
            let ([score1, score2], map) = &self.maps[map_id];

            let (medal1, medal2) = match score1.cmp(score2) {
                Ordering::Less => ("second", "first"),
                Ordering::Equal => ("first", "first"),
                Ordering::Greater => ("first", "second"),
            };

            let _ = writeln!(
                description,
                "**{i}.** [{title} [{version}]]({OSU_BASE}b/{map_id})\n\
                - :{medal1}_place: `{name1}`: {pp1:.2}pp :{medal2}_place: `{name2}`: {pp2:.2}pp",
                title = map.title,
                version = map.version,
                name1 = self.name1,
                pp1 = score1.pp,
                name2 = self.name2,
                pp2 = score2.pp,
            );
        }

        description.pop();

        let footer_text = format!(
            "🥇 count • {name1}: {wins1} • {name2}: {wins2}",
            name1 = self.name1,
            wins1 = self.wins[0],
            name2 = self.name2,
            wins2 = self.wins[1]
        );

        let embed = EmbedBuilder::new()
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(attachment("avatar_fuse.png"));

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
