use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::Countries;
use bathbot_psql::model::osu::{DbScoreBeatmap, DbScoreBeatmapset, DbTopScore, DbTopScores};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    osu::flag_url,
    CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        impls::scores::{MapFormatter, StarsFormatter},
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{RegionTopKind, ScoresOrder},
    core::{BotConfig, Context},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct RegionTopPagination {
    #[pagination(per_page = 10)]
    scores: DbTopScores<IntHasher>,
    mode: GameMode,
    sort: ScoresOrder,
    kind: RegionTopKind,
    msg_owner: Id<UserMarker>,
    content: Box<str>,
    pages: Pages,
}

impl IActiveMessage for RegionTopPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let data = &self.scores;

        let idx = self.pages.index();
        let scores = &data.scores()[idx..data.len().min(idx + self.pages.per_page())];

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();

        let mut footer_text = format!("Page {page}/{pages} • Mode: {}", mode_str(self.mode));

        if let RegionTopKind::Region { .. } = self.kind {
            footer_text += " • Region data provided by https://osuworld.octo.moe";
        }

        let footer = FooterBuilder::new(footer_text);

        let (title, thumbnail) = match &self.kind {
            RegionTopKind::Global => ("Global top 100 scores:".to_owned(), None),
            RegionTopKind::Country { country_code } => {
                let title = match Countries::code(country_code).to_name() {
                    Some(name) => {
                        let genitiv = if name.ends_with('s') { "" } else { "s" };

                        format!("{name}'{genitiv} top 100 scores:")
                    }
                    None => {
                        let genitiv = if country_code.ends_with('S') { "" } else { "s" };

                        format!("{country_code}'{genitiv} top 100 scores:")
                    }
                };

                (title, Some(flag_url(country_code)))
            }
            RegionTopKind::Region {
                country_code,
                region_name,
            } => {
                let title = format!("{region_name} ({country_code}) top 100 scores:");

                (title, Some(flag_url(country_code)))
            }
        };

        let config = BotConfig::get();
        let mut description = String::with_capacity(scores.len() * 160);

        if scores.is_empty() {
            description.push_str("No scores found");
        }

        for score in scores {
            let map = data.map(score.map_id);
            let mapset = map.and_then(|map| data.mapset(map.mapset_id));

            let _ = writeln!(
                description,
                "**#{pos} [{map}]({OSU_BASE}b/{map_id}) +{mods}**\n\
                by __[{user}]({OSU_BASE}u/{user_id})__ {grade} **{pp}pp**\
                {stars} • {acc}% • {appendix}",
                pos = score.pos,
                map = MapFormatter::new(map, mapset),
                map_id = score.map_id,
                mods = GameModsIntermode::from_bits(score.mods),
                user = score.username.cow_escape_markdown(),
                user_id = score.user_id,
                grade = config.grade(score.grade),
                pp = round(score.pp),
                stars = StarsFormatter::new(score.stars),
                acc = round(score.statistics.accuracy(self.mode)),
                appendix = OrderAppendix::new(self.sort, score, map, mapset),
            );
        }

        let mut embed = EmbedBuilder::new()
            .description(description)
            .footer(footer)
            .title(title);

        if let Some(thumbnail) = thumbnail {
            embed = embed.thumbnail(thumbnail);
        }

        BuildPage::new(embed, false)
            .content(self.content.clone())
            .boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
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

struct OrderAppendix<'s> {
    sort: ScoresOrder,
    score: &'s DbTopScore,
    map: Option<&'s DbScoreBeatmap>,
    mapset: Option<&'s DbScoreBeatmapset>,
}

impl<'s> OrderAppendix<'s> {
    fn new(
        sort: ScoresOrder,
        score: &'s DbTopScore,
        map: Option<&'s DbScoreBeatmap>,
        mapset: Option<&'s DbScoreBeatmapset>,
    ) -> Self {
        Self {
            sort,
            score,
            map,
            mapset,
        }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort {
            ScoresOrder::Ar => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .ar(self.map.map_or(0.0, |map| map.ar))
                    .build();

                write!(f, "`AR{}`", round(attrs.ar as f32))
            }
            ScoresOrder::Bpm => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let bpm = self.map.map_or(0.0, |map| map.bpm) * clock_rate;

                write!(f, "`{}BPM`", round(bpm))
            }
            ScoresOrder::Combo => write!(f, "`{}x`", self.score.max_combo),
            ScoresOrder::Cs => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .cs(self.map.map_or(0.0, |map| map.cs))
                    .build();

                write!(f, "`CS{}`", round(attrs.cs as f32))
            }
            ScoresOrder::Hp => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .hp(self.map.map_or(0.0, |map| map.hp))
                    .build();

                write!(f, "`HP{}`", round(attrs.hp as f32))
            }
            ScoresOrder::Length => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let seconds_drain = self.map.map_or(0, |map| map.seconds_drain) as f32 / clock_rate;

                write!(f, "`{}`", SecToMinSec::new(seconds_drain as u32))
            }
            ScoresOrder::Misses => write!(
                f,
                "{miss}{emote}",
                miss = self.score.statistics.miss,
                emote = Emote::Miss
            ),
            ScoresOrder::Od => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .od(self.map.map_or(0.0, |map| map.od))
                    .build();

                write!(f, "`OD{}`", round(attrs.od as f32))
            }
            ScoresOrder::RankedDate => {
                let ranked_date = self
                    .mapset
                    .and_then(|mapset| mapset.ranked_date)
                    .unwrap_or_else(OffsetDateTime::now_utc);

                write!(f, "<t:{}:R>", ranked_date.unix_timestamp())
            }
            ScoresOrder::Score => write!(f, "`{}`", WithComma::new(self.score.score)),
            ScoresOrder::Acc | ScoresOrder::Date | ScoresOrder::Pp | ScoresOrder::Stars => {
                write!(f, "<t:{}:R>", self.score.ended_at.unix_timestamp())
            }
        }
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "osu!",
        GameMode::Taiko => "Taiko",
        GameMode::Catch => "Catch",
        GameMode::Mania => "Mania",
    }
}
