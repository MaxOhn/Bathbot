use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::twilight_model::util::ImageHash;
use bathbot_psql::model::osu::{DbScore, DbScoreBeatmap, DbScoreBeatmapset, DbScoreUser, DbScores};
use bathbot_util::{
    constants::{MAP_THUMB_URL, OSU_BASE},
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::model::beatmap::BeatmapAttributesBuilder;
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
    core::BotConfig,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct ScoresMapPagination {
    #[pagination(per_page = 10)]
    scores: DbScores<IntHasher>,
    mode: Option<GameMode>,
    sort: ScoresOrder,
    guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for ScoresMapPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let data = &self.scores;

        // verified in command that these are available
        let (map_id, map) = data.maps().next().unwrap();
        let (mapset_id, mapset) = data.mapsets().next().unwrap();

        let author_text = format!(
            "{artist} - {title} [{version}]",
            artist = mapset.artist.cow_escape_markdown(),
            title = mapset.title.cow_escape_markdown(),
            version = map.version.cow_escape_markdown()
        );

        let icon_url = match self.guild_icon {
            Some((id, icon)) => format!(
                "https://cdn.discordapp.com/icons/{id}/{icon}.{ext}",
                ext = if icon.animated { "gif" } else { "webp" }
            ),
            // FIXME: MAP_THUMB_URL endpoint is sometimes wrong, see issue #426
            None => format!("{MAP_THUMB_URL}{mapset_id}l.jpg"),
        };

        let author = AuthorBuilder::new(author_text)
            .url(format!("{OSU_BASE}b/{map_id}"))
            .icon_url(icon_url);

        let footer_text = format!("Page {}/{}", pages.curr_page(), pages.last_page());
        let mut footer = FooterBuilder::new(footer_text);

        if let Some(mode) = self.mode {
            footer = footer.icon_url(Emote::from(mode).url());
        };

        let idx = pages.index();
        let scores = &data.scores()[idx..data.len().min(idx + pages.per_page())];

        let config = BotConfig::get();
        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter().zip(idx + 1..) {
            let mode = if self.mode.is_some() {
                None
            } else {
                Some(score.mode)
            };

            let _ = writeln!(
                description,
                "**#{i} [{user}]({OSU_BASE}u/{user_id})**: \
                {score} [ **{combo}x** ] **+{mods}**{stars}\n\
                {grade} **{pp}pp** • {acc}% {mode}{miss} {appendix}",
                grade = config.grade(score.grade),
                user = UserFormatter::new(data.user(score.user_id)),
                user_id = score.user_id,
                score = WithComma::new(score.score),
                combo = score.max_combo,
                mods = GameModsIntermode::from_bits(score.mods),
                stars = StarsFormatter::new(score.stars),
                pp = PpFormatter::new(score.pp),
                acc = round(score.statistics.accuracy(score.mode)),
                mode = GameModeFormatter::new(mode),
                miss = MissFormatter::new(score.statistics.count_miss),
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
    map: &'s DbScoreBeatmap,
    mapset: &'s DbScoreBeatmapset,
}

impl<'s> OrderAppendix<'s> {
    fn new(
        sort: ScoresOrder,
        score: &'s DbScore,
        map: &'s DbScoreBeatmap,
        mapset: &'s DbScoreBeatmapset,
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
                    .ar(self.map.ar, false)
                    .build();

                write!(f, "`AR{}`", round(attrs.ar as f32))
            }
            ScoresOrder::Bpm => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let bpm = self.map.bpm * clock_rate;

                write!(f, "`{}BPM`", round(bpm))
            }
            ScoresOrder::Cs => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .cs(self.map.cs, false)
                    .build();

                write!(f, "`CS{}`", round(attrs.cs as f32))
            }
            ScoresOrder::Hp => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .hp(self.map.hp, false)
                    .build();

                write!(f, "`HP{}`", round(attrs.hp as f32))
            }
            ScoresOrder::Length => {
                let clock_rate = GameModsIntermode::from_bits(self.score.mods).legacy_clock_rate();
                let seconds_drain = self.map.seconds_drain as f32 / clock_rate;

                write!(f, "`{}`", SecToMinSec::new(seconds_drain as u32))
            }
            ScoresOrder::Od => {
                let attrs = BeatmapAttributesBuilder::default()
                    .mods(self.score.mods)
                    .od(self.map.od, false)
                    .build();

                write!(f, "`OD{}`", round(attrs.od as f32))
            }
            ScoresOrder::RankedDate => {
                let ranked_date = self
                    .mapset
                    .ranked_date
                    .unwrap_or_else(OffsetDateTime::now_utc);

                write!(f, "<t:{}:R>", ranked_date.unix_timestamp())
            }
            ScoresOrder::Score => write!(f, "`{}`", WithComma::new(self.score.score)),
            ScoresOrder::Acc
            | ScoresOrder::Combo
            | ScoresOrder::Date
            | ScoresOrder::Misses
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
            Some(mode) => Display::fmt(&Emote::from(mode), f),
            None => Ok(()),
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

struct MissFormatter {
    misses: u32,
}

impl MissFormatter {
    fn new(misses: u32) -> Self {
        Self { misses }
    }
}

impl Display for MissFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.misses == 0 {
            return Ok(());
        }

        write!(
            f,
            " • {misses}{emote}",
            misses = self.misses,
            emote = Emote::Miss
        )
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
            .map_or(Ok(()), |stars| write!(f, " {stars}★"))
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
