use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_util::{
    constants::OSU_BASE, osu::flag_url, AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
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
    commands::osu::MedalEntryList,
    manager::redis::{ osu::CachedUser},
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsListPagination {
    user: CachedUser,
    acquired: (usize, usize),
    #[pagination(per_page = 10)]
    medals: Box<[MedalEntryList]>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsListPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let limit = self.medals.len().min(idx + pages.per_page());
        let medals = &self.medals[idx..limit];

        let mut description = String::with_capacity(1024);

        for (entry, i) in medals.iter().zip(pages.index() + 1..) {
            let url = match entry.medal.url() {
                Ok(url) => url,
                Err(err) => {
                    warn!(?err);

                    entry.medal.backup_url()
                }
            };

            let url = url.cow_replace("%25", "%");

            let _ = writeln!(
                description,
                "**#{i} [{medal}]({url})**\n\
                `{rarity:>5.2}%` • <t:{timestamp}:d> • {group}",
                medal = entry.medal.name,
                rarity = entry.rarity,
                timestamp = entry.achieved.unix_timestamp(),
                group = entry.medal.grouping,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Acquired {}/{} medals",
            self.acquired.0, self.acquired.1
        ));

        let country_code = self.user.country_code.as_str();
        let username = self.user.username.as_str();
        let user_id = self.user.user_id.to_native();
        let avatar_url = self.user.avatar_url.as_ref();

        let author = AuthorBuilder::new(username)
            .url(format!("{OSU_BASE}u/{user_id}"))
            .icon_url(flag_url(country_code));

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .thumbnail(avatar_url);

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
