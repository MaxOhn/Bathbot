use std::{fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::rosu_v2::user::User;
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
    core::Context,
    manager::redis::RedisData,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsListPagination {
    user: RedisData<User>,
    acquired: (usize, usize),
    #[pagination(per_page = 10)]
    medals: Box<[MedalEntryList]>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsListPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();
        let limit = self.medals.len().min(idx + pages.per_page());
        let medals = &self.medals[idx..limit];

        let mut description = String::with_capacity(1024);

        for (entry, i) in medals.iter().zip(pages.index() + 1..) {
            let _ = writeln!(
                description,
                "**#{i} [{medal}](https://osekai.net/medals/?medal={url_name})**\n\
                `{rarity:>5.2}%` • <t:{timestamp}:d> • {group}",
                medal = entry.medal.name,
                url_name = entry.medal.name.cow_replace(' ', "+"),
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

        let (country_code, username, user_id, avatar_url) = match self.user {
            RedisData::Original(ref user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_ref();

                (country_code, username, user_id, avatar_url)
            }
            RedisData::Archive(ref user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_ref();

                (country_code, username, user_id, avatar_url)
            }
        };

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
