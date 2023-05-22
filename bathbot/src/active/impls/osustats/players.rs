use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Write,
    sync::Arc,
};

use bathbot_model::{OsuStatsPlayer, OsuStatsPlayersArgs};
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    numbers::WithComma,
    osu::flag_url,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::{Result, WrapErr};
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
    core::Context,
    util::interaction::{InteractionComponent, InteractionModal},
};

pub struct OsuStatsPlayersPagination {
    players: HashMap<usize, Box<[OsuStatsPlayer]>, IntHasher>,
    params: OsuStatsPlayersArgs,
    first_place_id: u32,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for OsuStatsPlayersPagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
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

impl OsuStatsPlayersPagination {
    pub fn new(
        players: HashMap<usize, Box<[OsuStatsPlayer]>, IntHasher>,
        params: OsuStatsPlayersArgs,
        first_place_id: u32,
        amount: usize,
        content: String,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            players,
            params,
            first_place_id,
            content: content.into_boxed_str(),
            msg_owner,
            pages: Pages::new(15, amount),
        }
    }

    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let pages = &self.pages;
        let page = pages.curr_page();

        let players = match self.players.entry(page) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                self.params.page = page;

                let players = ctx
                    .client()
                    .get_country_globals(&self.params)
                    .await
                    .wrap_err("Failed to get country globals")?;

                e.insert(players.into_boxed_slice())
            }
        };

        let mut author = AuthorBuilder::new("Most global leaderboard scores");

        if let Some(ref country) = self.params.country {
            author = author.icon_url(flag_url(country.as_str()));
        }

        let mut description = String::with_capacity(1024);

        for (player, i) in players.iter().zip(pages.index() + 1..) {
            let _ = writeln!(
                description,
                "**{i}. [{}]({OSU_BASE}users/{})**: {}",
                player.username.cow_escape_markdown(),
                player.user_id,
                WithComma::new(player.count)
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let thumbnail = format!("{AVATAR_URL}{}", self.first_place_id);

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(thumbnail);

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}
