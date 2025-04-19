use std::{cmp::Ordering, collections::HashMap, fmt::Write};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{EmbedBuilder, FooterBuilder, IntHasher, constants::OSU_BASE};
use eyre::Result;
use rosu_v2::prelude::Username;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::osu::{CommonScore, CompareTopMap},
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
    async fn build_page(&mut self) -> Result<BuildPage> {
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
