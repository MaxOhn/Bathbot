use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    CowUtils, EmbedBuilder, FooterBuilder, ModsFormatter, ScoreExt,
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
    embeds::{ComboFormatter, HitResultFormatter, PpFormatter},
    manager::{redis::RedisData, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::{GradeFormatter, ScoreFormatter},
        Emote,
    },
};

pub struct TopPagination {
    user: RedisData<User>,
    mode: GameMode,
    entries: Box<[ScoreEmbedDataWrap]>,
    sort_by: TopScoreOrder,
    condensed_list: bool,
    score_data: ScoreData,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl TopPagination {
    pub fn builder() -> TopPaginationBuilder {
        TopPaginationBuilder {
            user: None,
            mode: None,
            entries: None,
            sort_by: None,
            condensed_list: None,
            score_data: None,
            content: None,
            msg_owner: None,
        }
    }

    fn build_condensed(&self) -> BuildPage {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());

        let scores = &self.entries[pages.index()..end_idx];

        let description = if self.mode == GameMode::Mania {
            self.condensed_description_mania(scores)
        } else {
            self.condensed_description(scores)
        };

        let footer_text = format!(
            "Page {}/{} • Mode: {}",
            self.pages.curr_page(),
            self.pages.last_page(),
            mode_str(self.mode)
        );

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url());

        BuildPage::new(embed, false).content(self.content.clone())
    }

    fn condensed_description(&self, entries: &[ScoreEmbedDataWrap]) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let entry = entry.get_half();

            let ScoreEmbedDataHalf {
                score,
                map,
                stars,
                max_combo,
                pb_idx,
                original_idx,
                ..
            } = entry;

            let _ = writeln!(
                description,
                "**#{idx} [{map}]({OSU_BASE}b/{map_id})** [{stars}★]\n\
                {grade} **{pp}pp** ({acc}%) [**{combo}x**/{max_combo}x] {miss}**+{mods}** {appendix}",
                idx = original_idx.or(pb_idx.as_ref().and_then(|idx| idx.idx)).expect("missing idx") + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                stars = round(*stars),
                grade = GradeFormatter::new(score.grade, Some(score.score_id), score.is_legacy()),
                pp = round(score.pp),
                acc = round(score.accuracy),
                combo = score.max_combo,
                miss = MissFormat(score.statistics.count_miss),
                mods = ModsFormatter::new(&score.mods),
                appendix = OrderAppendix::new(self.sort_by, entry, map.ranked_date(),  true, self.score_data),
            );
        }

        description
    }

    fn condensed_description_mania(&self, entries: &[ScoreEmbedDataWrap]) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let entry = entry.get_half();

            let ScoreEmbedDataHalf {
                score,
                map,
                stars,
                pb_idx,
                original_idx,
                ..
            } = entry;

            let stats = &score.statistics;

            let _ = writeln!(
                description,
                "**#{idx} [{map}]({OSU_BASE}b/{map_id})** [{stars}★]\n\
                {grade} **{pp}pp** {acc}% `{score}` {{{n320}/{n300}/../{miss}}} **+{mods}** {appendix}",
                idx = original_idx.or(pb_idx.as_ref().and_then(|idx| idx.idx)).expect("missing idx") + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                stars = round(*stars),
                grade = GradeFormatter::new(score.grade, Some(score.score_id), score.is_legacy()),
                pp = round(score.pp),
                acc = round(score.accuracy),
                // currently ignoring classic scoring, should it be considered for mania?
                score = ScoreFormat(score.score),
                n320 = stats.count_geki,
                n300 = stats.count_300,
                miss = stats.count_miss,
                mods = ModsFormatter::new(&score.mods),
                appendix =
                    OrderAppendix::new(self.sort_by, entry, map.ranked_date(), true, self.score_data),
            );
        }

        description
    }

    fn build_detailed(&self) -> BuildPage {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let scores = &self.entries[pages.index()..end_idx];

        let mut description = String::with_capacity(512);

        for entry in scores {
            let entry = entry.get_half();

            let ScoreEmbedDataHalf {
                score,
                map,
                max_pp,
                stars,
                max_combo,
                pb_idx,
                original_idx,
                ..
            } = entry;

            let _ = writeln!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{id}) +{mods}** [{stars:.2}★]\n\
                {grade} {pp} • {acc}% • {score}\n[ {combo} ] • {hits} • {appendix}",
                idx = original_idx
                    .or(pb_idx.as_ref().and_then(|idx| idx.idx))
                    .expect("missing idx")
                    + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(&score.mods),
                grade = GradeFormatter::new(score.grade, Some(score.score_id), score.is_legacy()),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = ScoreFormatter::new(score, self.score_data),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                appendix = OrderAppendix::new(
                    self.sort_by,
                    entry,
                    map.ranked_date(),
                    false,
                    self.score_data
                ),
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} • Mode: {}",
            self.pages.curr_page(),
            self.pages.last_page(),
            mode_str(self.mode)
        );

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url());

        BuildPage::new(embed, false).content(self.content.clone())
    }
}

impl IActiveMessage for TopPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        if self.condensed_list {
            self.build_condensed().boxed()
        } else {
            self.build_detailed().boxed()
        }
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

pub struct TopPaginationBuilder {
    user: Option<RedisData<User>>,
    mode: Option<GameMode>,
    entries: Option<Box<[ScoreEmbedDataWrap]>>,
    sort_by: Option<TopScoreOrder>,
    condensed_list: Option<bool>,
    score_data: Option<ScoreData>,
    content: Option<Box<str>>,
    msg_owner: Option<Id<UserMarker>>,
}

impl TopPaginationBuilder {
    pub fn build(&mut self) -> TopPagination {
        let user = self.user.take().expect("missing user");
        let mode = self.mode.expect("missing mode");
        let entries = self.entries.take().expect("missing entries");
        let sort_by = self.sort_by.expect("missing sort_by");
        let condensed_list = self.condensed_list.expect("missing condensed_list");
        let score_data = self.score_data.expect("missing score_data");
        let content = self.content.take().expect("missing content");
        let msg_owner = self.msg_owner.expect("missing msg_owner");

        let pages = if condensed_list {
            Pages::new(10, entries.len())
        } else {
            Pages::new(5, entries.len())
        };

        TopPagination {
            user,
            mode,
            entries,
            sort_by,
            condensed_list,
            score_data,
            content,
            msg_owner,
            pages,
        }
    }

    pub fn user(&mut self, user: RedisData<User>) -> &mut Self {
        self.user = Some(user);

        self
    }

    pub fn mode(&mut self, mode: GameMode) -> &mut Self {
        self.mode = Some(mode);

        self
    }

    pub fn entries(&mut self, entries: Box<[ScoreEmbedDataWrap]>) -> &mut Self {
        self.entries = Some(entries);

        self
    }

    pub fn sort_by(&mut self, sort_by: TopScoreOrder) -> &mut Self {
        self.sort_by = Some(sort_by);

        self
    }

    pub fn condensed_list(&mut self, condensed_list: bool) -> &mut Self {
        self.condensed_list = Some(condensed_list);

        self
    }

    pub fn score_data(&mut self, score_data: ScoreData) -> &mut Self {
        self.score_data = Some(score_data);

        self
    }

    pub fn content(&mut self, content: Box<str>) -> &mut Self {
        self.content = Some(content);

        self
    }

    pub fn msg_owner(&mut self, msg_owner: Id<UserMarker>) -> &mut Self {
        self.msg_owner = Some(msg_owner);

        self
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

struct MapFormat<'m> {
    map: &'m OsuMap,
}

impl<'m> MapFormat<'m> {
    fn new(map: &'m OsuMap) -> Self {
        Self { map }
    }
}

impl Display for MapFormat<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        const DASH_SPACE_SQUARE_BRACKETS: usize = 6;
        const SPACE_SQUARE_BRACKETS: usize = 3;
        const TRIPLE_DOTS: usize = 3;
        const MIN_LEN: usize = 15;
        const LIMIT: usize = 42;
        const MIN_LEN_WITH_DOTS: usize = MIN_LEN + TRIPLE_DOTS;

        let artist = self.map.artist().len();
        let title = self.map.title().len();
        let version = self.map.version().len();

        // if the dots wouldn't save space, might as well not replace the content
        let tuple = |pre, post| {
            if pre <= post + TRIPLE_DOTS {
                (pre, "")
            } else {
                (post, "...")
            }
        };

        if artist + title + version + DASH_SPACE_SQUARE_BRACKETS <= LIMIT {
            // short enough to display artist, title, and version
            write!(
                f,
                "{} - {} [{}]",
                self.map.artist().cow_escape_markdown(),
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title + version + SPACE_SQUARE_BRACKETS <= LIMIT {
            // show title and version without truncating
            write!(
                f,
                "{} [{}]",
                self.map.title().cow_escape_markdown(),
                self.map.version().cow_escape_markdown()
            )
        } else if version <= MIN_LEN_WITH_DOTS {
            // keep the version but truncate title
            let (end, suffix) = tuple(title, LIMIT - version - SPACE_SQUARE_BRACKETS - TRIPLE_DOTS);

            write!(
                f,
                "{}{suffix} [{}]",
                self.map.title()[..end].cow_escape_markdown(),
                self.map.version().cow_escape_markdown(),
            )
        } else if title <= MIN_LEN_WITH_DOTS {
            // keep the title but truncate version
            let (end, suffix) = tuple(version, LIMIT - title - SPACE_SQUARE_BRACKETS - TRIPLE_DOTS);

            write!(
                f,
                "{} [{}{suffix}]",
                self.map.title().cow_escape_markdown(),
                self.map.version()[..end].cow_escape_markdown(),
            )
        } else {
            // truncate title and version evenly
            let total_cut = title + version + SPACE_SQUARE_BRACKETS - LIMIT;
            let mut cut = total_cut / 2;

            let mut title_cut = match title.checked_sub(cut) {
                Some(len @ ..=MIN_LEN) => {
                    cut += MIN_LEN - len;

                    MIN_LEN
                }
                Some(len @ ..=MIN_LEN_WITH_DOTS) => {
                    cut += MIN_LEN_WITH_DOTS - len;

                    MIN_LEN
                }
                Some(len) => len - TRIPLE_DOTS,
                None => {
                    cut += cut - MIN_LEN;

                    MIN_LEN
                }
            };

            // if cut was off, increment by one
            cut += total_cut % 2;

            let version_cut = match version.checked_sub(cut) {
                Some(len @ ..=MIN_LEN) => {
                    title_cut -= MIN_LEN - len;

                    MIN_LEN
                }
                Some(len @ ..=MIN_LEN_WITH_DOTS) => {
                    title_cut -= MIN_LEN_WITH_DOTS - len;

                    MIN_LEN
                }
                Some(len) => len - TRIPLE_DOTS,
                None => {
                    title_cut -= cut - MIN_LEN;

                    MIN_LEN
                }
            };

            let (title_end, title_suffix) = tuple(title, title_cut);
            let (version_end, version_suffix) = tuple(version, version_cut);

            write!(
                f,
                "{}{title_suffix} [{}{version_suffix}]",
                self.map.title()[..title_end].cow_escape_markdown(),
                self.map.version()[..version_end].cow_escape_markdown(),
            )
        }
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(f, "{miss}{emote} ", miss = self.0, emote = Emote::Miss)
    }
}

struct ScoreFormat(u32);

impl Display for ScoreFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 < 10_000 {
            Display::fmt(&self.0, f)
        } else {
            write!(f, "{}K", self.0 / 1000)
        }
    }
}

pub struct OrderAppendix<'a> {
    sort_by: TopScoreOrder,
    entry: &'a ScoreEmbedDataHalf,
    ranked_date: Option<OffsetDateTime>,
    condensed: bool,
    score_data: ScoreData,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopScoreOrder,
        entry: &'a ScoreEmbedDataHalf,
        ranked_date: Option<OffsetDateTime>,
        condensed: bool,
        score_data: ScoreData,
    ) -> Self {
        Self {
            sort_by,
            entry,
            ranked_date,
            condensed,
            score_data,
        }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            TopScoreOrder::Ar => write!(f, "`AR {}`", round(self.entry.ar() as f32)),
            TopScoreOrder::Bpm => {
                let clock_rate = self.entry.score.mods.clock_rate().unwrap_or(1.0);

                write!(f, "`{}bpm`", round(self.entry.map.bpm() * clock_rate))
            }
            TopScoreOrder::Cs => write!(f, "`CS {}`", round(self.entry.cs() as f32)),
            TopScoreOrder::Length => {
                let clock_rate = self.entry.score.mods.clock_rate().unwrap_or(1.0);

                let secs = (self.entry.map.seconds_drain() as f32 / clock_rate) as u32;

                write!(f, "`{}:{:0>2}`", secs / 60, secs % 60)
            }
            TopScoreOrder::Hp => write!(f, "`HP {}`", round(self.entry.hp() as f32)),
            TopScoreOrder::Od => write!(f, "`OD {}`", round(self.entry.od() as f32)),
            TopScoreOrder::RankedDate => match self.ranked_date {
                Some(date) => write!(f, "<t:{}:d>", date.unix_timestamp()),
                None => Ok(()),
            },
            TopScoreOrder::Score if self.condensed && self.entry.map.mode() != GameMode::Mania => {
                let score = match self.score_data {
                    ScoreData::Stable | ScoreData::Lazer => self.entry.score.score,
                    ScoreData::LazerWithClassicScoring if self.entry.score.classic_score == 0 => {
                        self.entry.score.score
                    }
                    ScoreData::LazerWithClassicScoring => self.entry.score.classic_score,
                };

                if score > 1_000_000_000 {
                    write!(f, "`{:.2}bn`", score as f32 / 1_000_000_000.0)
                } else if score > 1_000_000 {
                    write!(f, "`{:.2}m`", score as f32 / 1_000_000.0)
                } else {
                    write!(f, "`{}`", WithComma::new(score))
                }
            }
            TopScoreOrder::Acc
            | TopScoreOrder::Combo
            | TopScoreOrder::Date
            | TopScoreOrder::Misses
            | TopScoreOrder::Pp
            | TopScoreOrder::Score
            | TopScoreOrder::Stars => HowLongAgoDynamic::new(&self.entry.score.ended_at).fmt(f),
        }
    }
}
