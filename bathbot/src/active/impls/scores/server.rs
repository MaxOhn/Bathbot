use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::twilight_model::util::ImageHash;
use bathbot_psql::model::osu::{DbScore, DbScoreBeatmap, DbScoreBeatmapset, DbScoreUser, DbScores};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::ScoresOrder,
    core::{BotConfig, Context},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct ScoresServerPagination {
    #[pagination(per_page = 10)]
    scores: DbScores<IntHasher>,
    mode: Option<GameMode>,
    sort: ScoresOrder,
    guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for ScoresServerPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let data = &self.scores;

        let mut author_text = "Server scores for ".to_owned();

        match self.mode {
            Some(GameMode::Osu) => author_text.push_str("osu!"),
            Some(GameMode::Taiko) => author_text.push_str("taiko"),
            Some(GameMode::Catch) => author_text.push_str("catch"),
            Some(GameMode::Mania) => author_text.push_str("mania"),
            None => author_text.push_str("all modes"),
        }

        author_text.push_str(" sorted by ");

        match self.sort {
            ScoresOrder::Acc => author_text.push_str("accuracy"),
            ScoresOrder::Ar => author_text.push_str("AR"),
            ScoresOrder::Bpm => author_text.push_str("BPM"),
            ScoresOrder::Combo => author_text.push_str("combo"),
            ScoresOrder::Cs => author_text.push_str("CS"),
            ScoresOrder::Date => author_text.push_str("date"),
            ScoresOrder::Hp => author_text.push_str("HP"),
            ScoresOrder::Length => author_text.push_str("length"),
            ScoresOrder::Misses => author_text.push_str("miss count"),
            ScoresOrder::Od => author_text.push_str("OD"),
            ScoresOrder::Pp => author_text.push_str("pp"),
            ScoresOrder::RankedDate => author_text.push_str("ranked date"),
            ScoresOrder::Score => author_text.push_str("score"),
            ScoresOrder::Stars => author_text.push_str("stars"),
        }

        author_text.push(':');

        let mut author = AuthorBuilder::new(author_text);

        if let Some((id, icon)) = self.guild_icon {
            let ext = if icon.animated { "gif" } else { "webp" };
            let url = format!("https://cdn.discordapp.com/icons/{id}/{icon}.{ext}");
            author = author.icon_url(url);
        }

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

        let config = BotConfig::get();
        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter().zip(idx + 1..) {
            let map = data.map(score.map_id);
            let mapset = map.and_then(|map| data.mapset(map.mapset_id));
            let user = data.user(score.user_id);

            let mode = if self.mode.is_some() {
                None
            } else {
                Some(score.mode)
            };

            let _ = writeln!(
                description,
                "**#{i} [{map}]({OSU_BASE}b/{map_id}) +{mods}**{stars}\n\
                by __[{user}]({OSU_BASE}u/{user_id})__ {grade} **{pp}pp** \
                • {acc}% {mode} {appendix}",
                map = MapFormatter::new(map, mapset),
                map_id = score.map_id,
                mods = GameModsIntermode::from_bits(score.mods),
                stars = StarsFormatter::new(score.stars),
                user = UserFormatter::new(user),
                user_id = score.user_id,
                grade = config.grade(score.grade),
                pp = PpFormatter::new(score.pp),
                acc = round(score.statistics.accuracy(score.mode)),
                mode = GameModeFormatter::new(mode),
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
                miss = self.score.statistics.count_miss,
                emote = Emote::Miss.text()
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

struct MapFormatter<'s> {
    map: Option<&'s DbScoreBeatmap>,
    mapset: Option<&'s DbScoreBeatmapset>,
}

impl<'s> MapFormatter<'s> {
    fn new(map: Option<&'s DbScoreBeatmap>, mapset: Option<&'s DbScoreBeatmapset>) -> Self {
        Self { map, mapset }
    }
}

impl Display for MapFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match (self.map, self.mapset) {
            (Some(map), Some(mapset)) => write!(
                f,
                "{artist} - {title} [{version}]",
                artist = mapset.artist.cow_escape_markdown(),
                title = mapset.title.cow_escape_markdown(),
                version = map.version.cow_escape_markdown()
            ),
            (Some(map), None) => write!(
                f,
                "<unknown mapset> [{version}]",
                version = map.version.cow_escape_markdown()
            ),
            (None, None) => f.write_str("<unknown map>"),
            (None, Some(_)) => unreachable!(),
        }
    }
}

struct UserFormatter<'s> {
    user: Option<&'s DbScoreUser>,
}

impl<'s> UserFormatter<'s> {
    fn new(user: Option<&'s DbScoreUser>) -> Self {
        Self { user }
    }
}

impl Display for UserFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.user {
            Some(user) => f.write_str(user.username.cow_escape_markdown().as_ref()),
            None => f.write_str("<unknown user>"),
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
            Some(mode) => f.write_str(Emote::from(mode).text()),
            None => f.write_str("•"),
        }
    }
}

struct StarsFormatter {
    stars: Option<f32>,
}

impl StarsFormatter {
    fn new(stars: Option<f32>) -> Self {
        Self { stars }
    }
}

impl Display for StarsFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.stars
            .map(round)
            .map_or(Ok(()), |stars| write!(f, " [{stars}★]"))
    }
}

struct PpFormatter {
    pp: Option<f32>,
}

impl PpFormatter {
    fn new(pp: Option<f32>) -> Self {
        Self { pp }
    }
}

impl Display for PpFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.pp.map(round) {
            Some(pp) => Display::fmt(&pp, f),
            None => f.write_str("-"),
        }
    }
}
