use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::{DbScore, DbScoreBeatmap, DbScoreBeatmapset, DbScores};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::SecToMinSec,
    numbers::{round, WithComma},
    osu::flag_url,
    AuthorBuilder, CowUtils, FooterBuilder, IntHasher,
};
use rosu_pp::beatmap::BeatmapAttributesBuilder;
use rosu_v2::prelude::{GameMode, GameModsIntermode};
use time::OffsetDateTime;

use crate::{
    commands::osu::ScoresOrder, core::BotConfig, manager::redis::RedisData, pagination::Pages,
    util::Emote,
};

#[derive(EmbedData)]
pub struct UserScoresEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
}

impl UserScoresEmbed {
    pub fn new(
        data: &DbScores<IntHasher>,
        user: &RedisData<User>,
        mode: Option<GameMode>,
        sort: ScoresOrder,
        pages: &Pages,
    ) -> Self {
        let author = if mode.is_some() {
            user.author_builder()
        } else {
            let icon = match user {
                RedisData::Original(user) => flag_url(&user.country_code),
                RedisData::Archive(user) => flag_url(&user.country_code),
            };

            let url = format!("{OSU_BASE}users/{}", user.user_id());

            AuthorBuilder::new(user.username()).url(url).icon_url(icon)
        };

        let footer_text = format!("Page {}/{}", pages.curr_page(), pages.last_page());
        let footer = FooterBuilder::new(footer_text);

        let idx = pages.index();
        let scores = &data.scores()[idx..data.len().min(idx + pages.per_page())];

        let config = BotConfig::get();
        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter().zip(idx + 1..) {
            let map = data.map(score.map_id);
            let mapset = map.and_then(|map| data.mapset(map.mapset_id));

            let mode = if mode.is_some() {
                None
            } else {
                Some(score.mode)
            };

            let _ = writeln!(
                description,
                "**{i}. [{map}]({OSU_BASE}b/{map_id})**{stars}\n\
                {grade} **{pp}pp** ({acc}%) [ **{combo}x** ] {mode}**+{mods}** {appendix}",
                map = MapFormatter::new(map, mapset),
                map_id = score.map_id,
                stars = StarsFormatter::new(score.stars),
                grade = config.grade(score.grade),
                pp = PpFormatter::new(score.pp),
                acc = round(score.statistics.accuracy(score.mode)),
                combo = score.max_combo,
                mode = GameModeFormatter::new(mode),
                mods = GameModsIntermode::from_bits(score.mods),
                appendix = OrderAppendix::new(sort, score, map, mapset),
            );
        }

        Self {
            author,
            description,
            footer,
        }
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
            Some(mode) => write!(f, "{} ", Emote::from(mode).text()),
            None => Ok(()),
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
            Some(pp) => write!(f, "{pp}"),
            None => f.write_str("-"),
        }
    }
}
