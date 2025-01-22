use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{constants::OSU_BASE, CowUtils, EmbedBuilder, FooterBuilder};
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
    manager::redis::osu::CachedUser,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        CachedUserExt,
    },
};

#[derive(PaginationBuilder)]
pub struct MostPlayedPagination {
    user: CachedUser,
    #[pagination(per_page = 10)]
    maps: Box<[MostPlayedMap]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MostPlayedPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.maps.len().min(pages.index() + pages.per_page());
        let maps = &self.maps[pages.index()..end_idx];

        let mut description = String::with_capacity(10 * 70);

        for most_played in maps {
            let map = &most_played.map;
            let mapset = &most_played.mapset;

            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id}) [{stars:.2}★]",
                count = most_played.count,
                title = mapset.title.cow_escape_markdown(),
                artist = mapset.artist.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                map_id = map.map_id,
                stars = map.stars,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url.as_ref())
            .title("Most played maps:");

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
