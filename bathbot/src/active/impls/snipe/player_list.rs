use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{rosu_v2::user::User, SnipeScore, SnipeScoreParams};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    osu::calculate_grade,
    CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use rosu_v2::prelude::{GameMode, ScoreStatistics};
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
    embeds::{HitResultFormatter, ModsFormatter, PpFormatter},
    manager::{redis::RedisData, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::grade_emote,
    },
};

#[derive(PaginationBuilder)]
pub struct SnipePlayerListPagination {
    user: RedisData<User>,
    #[pagination(per_page = 5, len = "total")]
    scores: BTreeMap<usize, SnipeScore>,
    maps: HashMap<u32, OsuMap, IntHasher>,
    total: usize,
    params: SnipeScoreParams,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for SnipePlayerListPagination {
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

impl SnipePlayerListPagination {
    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let pages = &self.pages;

        let count = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() && count < self.total - pages.index() {
            let huismetbenen_page = pages.index() / 50 + 1;
            self.params.page(huismetbenen_page as u8);

            // Get scores
            let scores = ctx
                .client()
                .get_national_firsts(&self.params)
                .await
                .wrap_err("Failed to get national firsts")?;

            // Store scores in BTreeMap
            let iter = scores
                .into_iter()
                .enumerate()
                .map(|(i, s)| ((huismetbenen_page - 1) * 50 + i, s));

            self.scores.extend(iter);
        }

        // Get maps from DB
        let map_ids: HashMap<_, _, _> = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page())
            .filter_map(|(_, score)| {
                if self.maps.contains_key(&score.map.map_id) {
                    None
                } else {
                    Some((score.map.map_id as i32, None))
                }
            })
            .collect();

        if !map_ids.is_empty() {
            let new_maps = match ctx.osu_map().maps(&map_ids).await {
                Ok(maps) => maps,
                Err(err) => {
                    warn!(?err, "Failed to get maps from database");

                    HashMap::default()
                }
            };

            self.maps.extend(new_maps);
        }

        if self.scores.is_empty() {
            let embed = EmbedBuilder::new()
                .author(self.user.author_builder())
                .description("No scores were found")
                .footer(FooterBuilder::new("Page 1/1 • Total #1 scores: 0"))
                .thumbnail(self.user.avatar_url());

            return Ok(BuildPage::new(embed, true).content(self.content.clone()));
        }

        let entries = self
            .scores
            .range(pages.index()..pages.index() + pages.per_page());

        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let map = self.maps.get(&score.map.map_id).expect("missing map");
            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();
            let max_pp = ctx.pp(map).mods(mods.bits()).performance().await.pp() as f32;

            let stats = ScoreStatistics {
                count_geki: 0,
                count_300: score.count_300.unwrap_or(0),
                count_katu: 0,
                count_100: score.count_100.unwrap_or(0),
                count_50: score.count_50.unwrap_or(0),
                count_miss: score.count_miss.unwrap_or(0),
            };

            let grade = calculate_grade(GameMode::Osu, mods.as_ref(), &stats);

            let _ = write!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{map_id}) {mods}** [{stars:.2}★]\n\
                {grade} {pp} • {acc}% • {score}\n\
                [ {combo} ] • {hits}",
                idx = idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = score.map.map_id,
                mods = ModsFormatter::new(&mods),
                stars = score.stars,
                grade = grade_emote(grade),
                pp = PpFormatter::new(score.pp, Some(max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, score.map.max_combo),
                hits = HitResultFormatter::new(GameMode::Osu, stats),
            );

            if let Some(ref date) = score.date_set {
                let _ = write!(description, " • {ago}", ago = HowLongAgoDynamic::new(date));
            }

            description.push('\n');
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Total scores: {}",
            self.total,
        ));

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}

struct ComboFormatter {
    combo: Option<u32>,
    max_combo: u32,
}

impl ComboFormatter {
    fn new(combo: Option<u32>, max_combo: u32) -> Self {
        Self { combo, max_combo }
    }
}

impl Display for ComboFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.combo {
            Some(combo) => write!(f, "**{combo}x**/")?,
            None => f.write_str("-/")?,
        }

        write!(f, "{}x", self.max_combo)
    }
}
