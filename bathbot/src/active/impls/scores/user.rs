use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::{DbScore, DbScoreBeatmap, DbScoreBeatmapset, DbScores};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    osu::flag_url,
    AuthorBuilder, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::model::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        impls::scores::{MapFormatter, PpFormatter, StarsFormatter},
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::ScoresOrder,
    manager::redis::RedisData,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeFormatter,
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct ScoresUserPagination {
    #[pagination(per_page = 10)]
    scores: DbScores<IntHasher>,
    user: RedisData<User>,
    mode: Option<GameMode>,
    sort: ScoresOrder,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for ScoresUserPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let author = if self.mode.is_some() {
            self.user.author_builder()
        } else {
            let icon = match self.user {
                RedisData::Original(ref user) => flag_url(&user.country_code),
                RedisData::Archive(ref user) => flag_url(&user.country_code),
            };

            let url = format!("{OSU_BASE}users/{}", self.user.user_id());

            AuthorBuilder::new(self.user.username())
                .url(url)
                .icon_url(icon)
        };

        let pages = &self.pages;
        let data = &self.scores;

        let mut footer_text = format!("Page {}/{}", pages.curr_page(), pages.last_page());

        if let Some(mode) = self.mode {
            footer_text.push_str(" • Mode: ");

            let mode = match mode {
                GameMode::Osu => "osu!",
                GameMode::Taiko => "Taiko",
                GameMode::Catch => "Catch",
                GameMode::Mania => "Mania",
            };

            footer_text.push_str(mode);
        }

        let footer = FooterBuilder::new(footer_text);

        let idx = pages.index();
        let scores = &data.scores()[idx..data.len().min(idx + pages.per_page())];

        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter().zip(idx + 1..) {
            let map = data.map(score.map_id);
            let mapset = map.and_then(|map| data.mapset(map.mapset_id));

            let mode = if self.mode.is_some() {
                None
            } else {
                Some(score.mode)
            };

            let _ = writeln!(
                description,
                "**#{i} [{map}]({OSU_BASE}b/{map_id})**{stars}\n\
                {grade} **{pp}pp** • {acc}% [ **{combo}x** ] \
                {mode}**+{mods}** {appendix}",
                map = MapFormatter::new(map, mapset),
                map_id = score.map_id,
                stars = StarsFormatter::new(score.stars),
                grade = GradeFormatter::new(score.grade, None, false),
                pp = PpFormatter::new(score.pp),
                acc = round(score.statistics.accuracy(score.mode)),
                combo = score.max_combo,
                mode = GameModeFormatter::new(mode),
                mods = GameModsIntermode::from_bits(score.mods),
                appendix = OrderAppendix::new(self.sort, score, map, mapset),
            );
        }

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer);

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

struct OrderAppendix<'s> {
    sort: ScoresOrder,
    score: &'s DbScore,
    map: Option<&'s DbScoreBeatmap>,
    mapset: Option<&'s DbScoreBeatmapset>,
}

impl<'s> OrderAppendix<'s> {
    fn new(
        sort: ScoresOrder,
        score: &'s DbScore,
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
                    .ar(self.map.map_or(0.0, |map| map.ar), false)
                    .build();

                write!(f, "`AR{}`", round(attrs.ar as f32))
            }
            ScoresOrder::Bpm => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let bpm = self.map.map_or(0.0, |map| map.bpm) * clock_rate as f32;

                write!(f, "`{}BPM`", round(bpm))
            }
            ScoresOrder::Cs => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .cs(self.map.map_or(0.0, |map| map.cs), false)
                    .build();

                write!(f, "`CS{}`", round(attrs.cs as f32))
            }
            ScoresOrder::Hp => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .hp(self.map.map_or(0.0, |map| map.hp), false)
                    .build();

                write!(f, "`HP{}`", round(attrs.hp as f32))
            }
            ScoresOrder::Length => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let seconds_drain = self.map.map_or(0, |map| map.seconds_drain) as f64 / clock_rate;

                write!(f, "`{}`", SecToMinSec::new(seconds_drain as u32))
            }
            ScoresOrder::Misses => write!(
                f,
                "{miss}{emote}",
                miss = self.score.statistics.count_miss,
                emote = Emote::Miss
            ),
            ScoresOrder::Od => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .od(self.map.map_or(0.0, |map| map.od), false)
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
            ScoresOrder::Acc
            | ScoresOrder::Combo
            | ScoresOrder::Date
            | ScoresOrder::Pp
            | ScoresOrder::Stars => {
                write!(f, "<t:{}:R>", self.score.ended_at.unix_timestamp())
            }
        }
    }
}

struct GameModeFormatter {
    mode: Option<GameMode>,
}

impl GameModeFormatter {
    fn new(mode: Option<GameMode>) -> Self {
        Self { mode }
    }
}

impl Display for GameModeFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.mode {
            Some(mode) => write!(f, "{} ", Emote::from(mode)),
            None => Ok(()),
        }
    }
}
