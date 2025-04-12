use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{RelaxPlayersDataResponse, RelaxScore};
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, IntHasher, ModsFormatter,
    constants::{OSU_BASE, RELAX_ICON_URL},
    datetime::HowLongAgoDynamic,
    numbers::{WithComma, round},
};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_interactions::command::{CommandOption, CreateOption};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::osu::relax::relax_author_builder,
    core::Context,
    embeds::{ComboFormatter, PpFormatter},
    manager::{OsuMap, redis::osu::CachedUser},
    util::{
        Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};
#[derive(PaginationBuilder)]
pub struct RelaxTopPagination {
    user: CachedUser,
    relax_user: RelaxPlayersDataResponse,
    #[pagination(per_page = 5)]
    scores: Vec<RelaxScore>,
    maps: HashMap<u32, OsuMap, IntHasher>,
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

        if self.scores.is_empty() {
            let embed = EmbedBuilder::new()
                .author(relax_author_builder(&self.user, &self.relax_user))
                .description("No scores were found")
                .footer(
                    FooterBuilder::new("Page 1/1 • Total #1 scores: 0").icon_url(RELAX_ICON_URL),
                )
                .thumbnail(self.user.avatar_url.as_ref());

            return Ok(BuildPage::new(embed, true));
        }

        let map_ids: HashMap<_, _, _> = self
            .scores
            .iter()
            .skip(pages.index())
            .take(pages.per_page())
            .filter_map(|score| {
                if self.maps.contains_key(&score.beatmap_id) {
                    None
                } else {
                    Some((score.beatmap_id as i32, None))
                }
            })
            .collect();

        if !map_ids.is_empty() {
            match Context::osu_map().maps(&map_ids).await {
                Ok(new_maps) => self.maps.extend(new_maps),
                Err(err) => warn!(?err, "Failed to get maps from database"),
            };
        }

        let entries = self
            .scores
            .iter()
            .enumerate()
            .skip(pages.index())
            .take(pages.per_page());

        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let Some(map) = self.maps.get(&score.beatmap_id) else {
                warn!("Missing map");
                continue;
            };

            let mods = &score.mods;
            let mut pp_manager = Context::pp(map).mods(mods.clone());
            let max_attrs = pp_manager.performance().await;

            // NOTE: Make generic versions of formatting functions later on
            // this is ugly
            let score_pp = score.pp.map(|pp| pp as f32);
            let max_pp = max_attrs.pp() as f32;
            let max_combo = max_attrs.max_combo();
            let count_miss = score.count_miss;

            let _ = writeln!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {pp} • {acc}% • [{stars:.2}★]{miss}\n\
                [ {combo} ] • {score} • {date}",
                idx = idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = score.beatmap_id,
                mods = ModsFormatter::new(mods),
                pp = PpFormatter::new(score_pp, Some(max_pp)),
                stars = max_attrs.stars(),
                acc = round(score.accuracy),
                score = WithComma::new(score.total_score),
                combo = ComboFormatter::new(score.combo, Some(max_combo)),
                miss = MissFormat(count_miss),
                date = HowLongAgoDynamic::new(&score.date),
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Total scores: {}",
            self.scores.len(),
        ));

        let embed = EmbedBuilder::new()
            .author(relax_author_builder(&self.user, &self.relax_user))
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true))
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(f, " • {miss}{emote}", miss = self.0, emote = Emote::Miss)
    }
}
#[derive(Copy, Clone, CommandOption, CreateOption, Default, Eq, PartialEq)]
pub enum RelaxTopOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Misses", value = "misses")]
    Misses,
    #[option(name = "Mods count", value = "mods_count")]
    ModsCount,
    #[default]
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
}
