use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{CowUtils, EmbedBuilder, FooterBuilder, constants::OSU_BASE};
use eyre::Result;
use rosu_v2::prelude::MostPlayedMap;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    manager::redis::osu::CachedUser,
    util::{
        CachedUserExt,
        interaction::{InteractionComponent, InteractionModal},
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
    async fn build_page(&mut self) -> Result<BuildPage> {
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
            .author(self.user.author_builder(false))
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url.as_ref())
            .title("Most played maps:");

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
