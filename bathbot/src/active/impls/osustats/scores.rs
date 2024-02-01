use std::{collections::BTreeMap, fmt::Write, sync::Arc};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{rosu_v2::user::User, OsuStatsParams, OsuStatsScoresRaw, ScoreSlim};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    CowUtils, EmbedBuilder, FooterBuilder, ModsFormatter,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::{
    model::score::LegacyScoreStatistics,
    prelude::{GameMode, Grade},
};
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::OsuStatsEntry,
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, PpFormatter},
    manager::redis::RedisData,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::grade_emote,
    },
};

#[derive(PaginationBuilder)]
pub struct OsuStatsScoresPagination {
    user: RedisData<User>,
    #[pagination(per_page = 5, len = "total")]
    entries: BTreeMap<usize, OsuStatsEntry>,
    total: usize,
    params: OsuStatsParams,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for OsuStatsScoresPagination {
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

impl OsuStatsScoresPagination {
    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let pages = &self.pages;

        let entries = self
            .entries
            .range(pages.index()..pages.index() + pages.per_page());
        let count = entries.count();

        if count < pages.per_page() && self.total - pages.index() > count {
            let osustats_page = (pages.index() / 24) + 1;
            self.params.page = osustats_page;
            let scores_fut = ctx.client().get_global_scores(&self.params);

            let scores = match scores_fut.await.map(OsuStatsScoresRaw::into_scores) {
                Ok(Ok(scores)) => scores.scores,
                Err(err) | Ok(Err(err)) => return Err(err.wrap_err("Failed to get global scores")),
            };

            let maps_id_checksum = scores
                .iter()
                .map(|score| (score.map.map_id as i32, None))
                .collect();

            let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;
            let mode = self.params.mode;

            for (score, i) in scores.into_iter().zip((osustats_page - 1) * 24..) {
                let map_opt = maps.remove(&score.map.map_id);
                let Some(map) = map_opt else { continue };

                let mut calc = ctx.pp(&map).mods(&score.mods).mode(mode);
                let attrs = calc.performance().await;

                let pp = match score.pp {
                    Some(pp) => pp,
                    None => calc.score(&score).performance().await.pp() as f32,
                };

                let max_pp =
                    if score.grade.eq_letter(Grade::X) && mode != GameMode::Mania && pp > 0.0 {
                        pp
                    } else {
                        attrs.pp() as f32
                    };

                let rank = score.position;

                let score = ScoreSlim {
                    accuracy: score.accuracy,
                    ended_at: score.ended_at,
                    grade: score.grade,
                    max_combo: score.max_combo,
                    mode,
                    mods: score.mods,
                    pp,
                    score: score.score,
                    score_id: None,
                    statistics: LegacyScoreStatistics {
                        count_geki: score.count_geki,
                        count_300: score.count300,
                        count_katu: score.count_katu,
                        count_100: score.count100,
                        count_50: score.count50,
                        count_miss: score.count_miss,
                    },
                };

                let entry = OsuStatsEntry {
                    score,
                    map,
                    rank,
                    max_pp,
                    stars: attrs.stars() as f32,
                    max_combo: attrs.max_combo() as u32,
                };

                self.entries.insert(i, entry);
            }
        }

        if self.entries.is_empty() {
            let embed = EmbedBuilder::new()
                .author(self.user.author_builder())
                .description("No scores with these parameters were found")
                .footer(FooterBuilder::new("Page 1/1 • Total scores: 0"))
                .thumbnail(self.user.avatar_url());

            return Ok(BuildPage::new(embed, true).content(self.content.clone()));
        }

        let page = pages.curr_page();
        let per_page = pages.per_page();
        let index = pages.index();
        let pages = pages.last_page();

        let entries = self.entries.range(index..index + per_page);
        let mut description = String::with_capacity(1024);

        for (_, entry) in entries {
            let OsuStatsEntry {
                score,
                map,
                rank,
                stars,
                max_pp,
                max_combo,
            } = entry;

            let grade = grade_emote(score.grade);

            let _ = writeln!(
                description,
                "**#{rank} [{title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}** [{stars:.2}★]\n\
                {grade} {pp} • {acc}% • {score}\n\
                [ {combo} ] • {hits} • {ago}",
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = map.map_id(),
                mods = ModsFormatter::new(&score.mods),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                ago = HowLongAgoDynamic::new(&score.ended_at),
            );
        }

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} • Total scores: {}",
            self.total
        ));

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url());

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}
