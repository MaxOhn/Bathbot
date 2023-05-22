use std::{cmp::Ordering, collections::HashMap, fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{constants::OSU_BASE, CowUtils, EmbedBuilder, IntHasher};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::MostPlayedMap;
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
pub struct CompareMostPlayedPagination {
    username1: Box<str>,
    username2: Box<str>,
    #[pagination(per_page = 10)]
    maps: HashMap<u32, ([usize; 2], MostPlayedMap), IntHasher>,
    map_counts: Box<[(u32, usize)]>,
    content: String,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for CompareMostPlayedPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let map_counts = &self.map_counts[idx..self.maps.len().min(idx + pages.per_page())];

        let mut description = String::with_capacity(512);

        for ((map_id, _), i) in map_counts.iter().zip(pages.index() + 1..) {
            let ([count1, count2], map) = &self.maps[map_id];

            let (medal1, medal2) = match count1.cmp(count2) {
                Ordering::Less => ("second", "first"),
                Ordering::Equal => ("first", "first"),
                Ordering::Greater => ("first", "second"),
            };

            let _ = writeln!(
                description,
                "**{i}.** [{title} [{version}]]({OSU_BASE}b/{map_id}) [{stars:.2}★]\n\
                - :{medal1}_place: `{name1}`: **{count1}** :{medal2}_place: `{name2}`: **{count2}**",
                title = map.mapset.title.cow_escape_markdown(),
                version = map.map.version.cow_escape_markdown(),
                stars = map.map.stars,
                name1 = self.username1,
                name2 = self.username2,
            );
        }

        description.pop();

        let embed = EmbedBuilder::new().description(description);

        BuildPage::new(embed, false)
            .content(self.content.clone())
            .boxed()
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
