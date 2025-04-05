use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::RelaxScore;
use bathbot_util::{
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher, ModsFormatter,
    constants::{OSU_BASE, RELAX},
    datetime::HowLongAgoDynamic,
    numbers::{WithComma, round},
    osu::flag_url,
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
    core::Context,
    embeds::{ComboFormatter, PpFormatter},
    manager::{OsuMap, redis::osu::CachedUser},
    util::{
        CachedUserExt, Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};
#[derive(PaginationBuilder)]
pub struct RelaxTopPagination {
    user: CachedUser,
    #[pagination(per_page = 5, len = "total")]
    scores: Vec<RelaxScore>,
    total: usize,
    maps: HashMap<u32, OsuMap, IntHasher>,
    sort: RelaxTopOrder,
    // condensed_list: bool,
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

        if self.scores.is_empty() {
            let embed = EmbedBuilder::new()
                .author(author_builder(&self.user))
                .description("No scores were found")
                .footer(FooterBuilder::new("Page 1/1 • Total #1 scores: 0"))
                .thumbnail(self.user.avatar_url.as_ref());

            return Ok(BuildPage::new(embed, true).content(self.content.clone()));
        }

        match self.sort {
            RelaxTopOrder::Acc => self.scores.sort_unstable_by(|lhs, rhs| {
                rhs.accuracy
                    .partial_cmp(&lhs.accuracy)
                    .unwrap_or(Ordering::Equal)
            }),
            RelaxTopOrder::Bpm => self.scores.sort_unstable_by(|lhs, rhs| {
                rhs.beatmap
                    .beats_per_minute
                    .total_cmp(&lhs.beatmap.beats_per_minute)
            }),
            RelaxTopOrder::Combo => self
                .scores
                .sort_unstable_by(|lhs, rhs| rhs.combo.cmp(&lhs.combo)),
            RelaxTopOrder::Date => self
                .scores
                .sort_unstable_by(|lhs, rhs| rhs.date.cmp(&lhs.date)),
            RelaxTopOrder::Misses => self
                .scores
                .sort_unstable_by(|lhs, rhs| rhs.count_miss.cmp(&lhs.count_miss)),
            RelaxTopOrder::ModsCount => self
                .scores
                .sort_unstable_by(|lhs, rhs| rhs.mods.len().cmp(&lhs.mods.len())),
            RelaxTopOrder::Pp => self.scores.sort_unstable_by(|lhs, rhs| {
                rhs.pp.partial_cmp(&lhs.pp).unwrap_or(Ordering::Equal)
            }),
            RelaxTopOrder::Score => self
                .scores
                .sort_unstable_by(|lhs, rhs| rhs.total_score.cmp(&lhs.total_score)),
            RelaxTopOrder::Stars => self.scores.sort_unstable_by(|lhs, rhs| {
                rhs.beatmap
                    .star_rating_normal
                    .total_cmp(&lhs.beatmap.star_rating_normal)
            }),
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
            let new_maps = match Context::osu_map().maps(&map_ids).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!(?err, "Failed to get maps from database");

                    HashMap::default()
                }
            };

            self.maps.extend(new_maps);
        }

        let entries = self
            .scores
            .iter()
            .enumerate()
            .skip(pages.index())
            .take(pages.per_page());

        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let map = self.maps.get(&score.beatmap_id).expect("Missing map");
            let mods = Cow::Borrowed(&score.mods);
            let max_attrs = Context::pp(map)
                .mods(mods.clone().into_owned())
                .performance()
                .await;
            // NOTE: Make generic versions of formatting functions later on
            // this is ugly
            let score_pp = score.pp.map(|pp| pp as f32);
            let max_pp = max_attrs.pp() as f32;
            let max_combo = max_attrs.max_combo();
            let count_miss = score.count_miss;

            let _ = write!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {pp} • {acc}% • [{stars:.2}★]{miss}\n\
                [ {combo} ] • {score} \n\
                {date}",
                idx = idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = score.beatmap_id,
                mods = ModsFormatter::new(&mods),
                pp = PpFormatter::new(score_pp, Some(max_pp)),
                stars = score.beatmap.star_rating.unwrap_or_default(),
                acc = round(score.accuracy),
                score = WithComma::new(score.total_score),
                combo = ComboFormatter::new(score.combo, Some(max_combo)),
                miss = MissFormat(count_miss),
                date = HowLongAgoDynamic::new(&score.date),
            );

            description.push('\n');
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Total scores: {}",
            self.total,
        ));

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(false))
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}

fn author_builder(user: &CachedUser) -> AuthorBuilder {
    let text = format!("{name}", name = user.username);

    let url = format!("{RELAX}/users/{}", user.user_id);
    let icon = flag_url(&user.country_code);

    AuthorBuilder::new(text).url(url).icon_url(icon)
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
    #[option(name = "Stars", value = "stars")]
    Stars,
}
