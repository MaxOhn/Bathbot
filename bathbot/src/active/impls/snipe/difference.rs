use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    fmt::Write,
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{rosu_v2::user::User, SnipeRecent};
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::round, CowUtils, EmbedBuilder,
    FooterBuilder, IntHasher,
};
use eyre::{Result, WrapErr};
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
    commands::osu::Difference,
    core::{Context, ContextExt},
    embeds::ModsFormatter,
    manager::redis::RedisData,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct SnipeDifferencePagination {
    user: RedisData<User>,
    diff: Difference,
    #[pagination(per_page = 10)]
    scores: Box<[SnipeRecent]>,
    star_map: HashMap<u32, f32, IntHasher>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SnipeDifferencePagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
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

impl SnipeDifferencePagination {
    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let pages = &self.pages;
        let mut description = String::with_capacity(512);

        // not necessary but less ugly than the iterator
        #[allow(clippy::needless_range_loop)]
        for idx in pages.index()..self.scores.len().min(pages.index() + pages.per_page()) {
            let score = &self.scores[idx];

            let stars = match score.stars {
                Some(stars) => *self
                    .star_map
                    .entry(score.map_id)
                    .and_modify(|entry| *entry = stars)
                    .or_insert(stars),
                None => match self.star_map.entry(score.map_id) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        let map = ctx
                            .osu_map()
                            .pp_map(score.map_id)
                            .await
                            .wrap_err("Failed to get pp map")?;

                        let stars = ctx
                            .pp_parsed(&map, score.map_id, GameMode::Osu)
                            .difficulty()
                            .await
                            .stars();

                        *e.insert(stars as f32)
                    }
                },
            };

            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();

            let _ = write!(
                description,
                "**#{idx} [{artist} - {title} [{version}]]({OSU_BASE}b/{id}) {mods}**\n\
                [{stars:.2}★] • {acc}% • ",
                idx = idx + 1,
                artist = score.artist.cow_escape_markdown(),
                title = score.title.cow_escape_markdown(),
                version = score.version.cow_escape_markdown(),
                id = score.map_id,
                mods = ModsFormatter::new(mods.as_ref()),
                acc = round(score.accuracy),
            );

            let _ = match self.diff {
                Difference::Gain => match score.sniped.as_deref().zip(score.sniped_id) {
                    Some((name, user_id)) => write!(
                        description,
                        "Sniped [{name}]({OSU_BASE}u/{user_id}) ",
                        name = name.cow_escape_markdown(),
                    ),
                    None => write!(description, "Unclaimed until "),
                },
                Difference::Loss => match score.sniper.as_deref() {
                    // should technically always be `Some` but huismetbenen is bugged
                    Some(name) => write!(
                        description,
                        "Sniped by [{name}]({OSU_BASE}u/{user_id}) ",
                        name = name.cow_escape_markdown(),
                        user_id = score.sniper_id,
                    ),
                    None => write!(
                        description,
                        "Sniped by [<unknown user>]({OSU_BASE}u/{})",
                        score.sniper_id
                    ),
                },
            };

            if let Some(ref date) = score.date {
                let _ = write!(description, "{}", HowLongAgoDynamic::new(date));
            } else {
                description.push_str("<unknown date>");
            }

            description.push('\n');
        }

        description.pop();

        let title = match self.diff {
            Difference::Gain => "New national #1s since last week",
            Difference::Loss => "Lost national #1s since last week",
        };

        let footer = FooterBuilder::new(format!(
            "Page {}/{} • Total: {}",
            self.pages.curr_page(),
            self.pages.last_page(),
            self.scores.len()
        ));

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url())
            .title(title);

        Ok(BuildPage::new(embed, true))
    }
}
