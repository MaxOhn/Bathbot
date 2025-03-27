use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{RelaxScore, RelaxUser};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::RELAX,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    osu::flag_url,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher, ModsFormatter, ScoreExt,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::{
        osu::TopScoreOrder,
        utility::{ScoreEmbedDataHalf, ScoreEmbedDataWrap},
    },
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, PpFormatter},
    manager::{redis::osu::CachedUser, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::{GradeFormatter, ScoreFormatter},
        CachedUserExt, Emote,
    },
};
#[derive(PaginationBuilder)]
pub struct RelaxTopPagination {
    user: CachedUser,
    sort_by: TopScoreOrder,
    #[pagination(per_page = 5, len = "total")]
    scores: BTreeMap<usize, RelaxScore>,
    total: usize,
    maps: HashMap<u32, OsuMap, IntHasher>,
    condensed_list: bool,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for RelaxTopPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page())
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,

        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, true, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, true, &mut self.pages)
    }
}

impl RelaxTopPagination {
    async fn async_build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let page_range = pages.index()..pages.index() + pages.per_page();

        let page_count = self.scores.range(page_range.clone()).count();

        if self.scores.is_empty() {
            let embed = EmbedBuilder::new()
                .author(author_builder(&self.user))
                .description("No scores were found")
                .footer(FooterBuilder::new("Page 1/1 • Total #1 scores: 0"))
                .thumbnail(self.user.avatar_url.as_ref());

            return Ok(BuildPage::new(embed, true).content(self.content.clone()));
        }

        let map_ids: HashMap<_, _, _> = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .filter_map(|(_, score)| {
                if self.maps.contains_key(&score.beatmap_id) {
                    None
                } else {
                    Some((score.beatmap_id as i32, None))
                }
            })
            .collect();

        if !map_ids.is_empty() {
            let new_maps = match Context::osu_map().maps(&map_ids).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!(?err, "Failed to get maps from database");

                    HashMap::default()
                }
            };

            self.maps.extend(new_maps);
        }

        let entries = self.scores.range(page_range.clone());

        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let map = self.maps.get(&score.beatmap_id).expect("Missing map");
            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();
        }

        todo!()
    }
}

fn author_builder(user: &CachedUser) -> AuthorBuilder {
    let text = format!("{name}", name = user.username,);

    let url = format!("{RELAX}/users/{}", user.id);
    let icon = flag_url(&user.country_code);

    AuthorBuilder::new(text).url(url).icon_url(icon)
}
