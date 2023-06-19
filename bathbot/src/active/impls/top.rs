use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_model::{rosu_v2::user::User, OsuTrackerMapsetEntry};
use bathbot_psql::model::configs::{ListSize, MinimizedPp};
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    fields,
    numbers::{round, WithComma},
    CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
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
    commands::osu::{TopEntry, TopScoreOrder},
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, KeyFormatter, ModsFormatter, PpFormatter},
    manager::{redis::RedisData, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::{grade_completion_mods, grade_emote, IfFc},
        Emote,
    },
};

type Farm = HashMap<u32, (OsuTrackerMapsetEntry, bool), IntHasher>;

pub struct TopPagination {
    user: RedisData<User>,
    mode: GameMode,
    entries: Box<[TopEntry]>,
    sort_by: TopScoreOrder,
    farm: Farm,
    list_size: ListSize,
    minimized_pp: MinimizedPp, // only relevant for `ListSize::Single`
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
            farm: None,
            list_size: None,
            minimized_pp: None,
            content: None,
            msg_owner: None,
        }
    }

    fn build_condensed(&mut self) -> BuildPage {
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

    fn condensed_description(&self, entries: &[TopEntry]) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                stars,
                max_pp: _,
                max_combo,
            } = entry;

            let _ = writeln!(
                description,
                "**#{idx} [{map}]({OSU_BASE}b/{map_id})** [{stars}★]\n\
                {grade} **{pp}pp** ({acc}%) [**{combo}x**/{max_combo}x] {miss}**+{mods}** {appendix}",
                idx = *original_idx + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                stars = round(*stars),
                grade = grade_emote(score.grade),
                pp = round(score.pp),
                acc = round(score.accuracy),
                combo = score.max_combo,
                miss = MissFormat(score.statistics.count_miss),
                mods = score.mods,
                appendix = OrderAppendix::new(self.sort_by, entry, map.ranked_date(), &self.farm, true),
            );
        }

        description
    }

    fn condensed_description_mania(&self, entries: &[TopEntry]) -> String {
        let mut description = String::with_capacity(1024);

        for entry in entries {
            let TopEntry {
                original_idx,
                score,
                map,
                max_pp: _,
                max_combo: _,
                stars: _,
            } = entry;

            let stats = &score.statistics;

            let _ = writeln!(
                description,
                "**#{idx} [{map}]({OSU_BASE}b/{map_id}) +{mods}**\n\
                {grade} **{pp}pp** ({acc}%) `{score}` {{{n320}/{n300}/.../{miss}}} {appendix}",
                idx = *original_idx + 1,
                map = MapFormat::new(map),
                map_id = map.map_id(),
                mods = score.mods,
                grade = grade_emote(score.grade),
                pp = round(score.pp),
                acc = round(score.accuracy),
                score = ScoreFormat(score.score),
                n320 = stats.count_geki,
                n300 = stats.count_300,
                miss = stats.count_miss,
                appendix =
                    OrderAppendix::new(self.sort_by, entry, map.ranked_date(), &self.farm, true),
            );
        }

        description
    }

    fn build_detailed(&mut self) -> BuildPage {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let scores = &self.entries[pages.index()..end_idx];

        let mut description = String::with_capacity(512);

        for entry in scores {
            let TopEntry {
                original_idx,
                score,
                map,
                max_pp,
                max_combo,
                stars,
            } = entry;

            let _ = writeln!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {pp} • {acc}% • {score}\n[ {combo} ] • {hits} • {appendix}",
                idx = *original_idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(&score.mods),
                grade = grade_emote(score.grade),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                appendix =
                    OrderAppendix::new(self.sort_by, entry, map.ranked_date(), &self.farm, false),
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

    async fn build_single(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let entry = &self.entries[self.pages.index()];

        // Required for /pinned
        let personal_idx = (entry.original_idx != usize::MAX).then_some(entry.original_idx);

        let TopEntry {
            original_idx: _, // use personal_idx instead so this works for pinned aswell
            score,
            map,
            max_pp,
            max_combo,
            stars,
        } = entry;

        let if_fc = IfFc::new(&ctx, score, map).await;
        let hits = HitResultFormatter::new(score.mode, score.statistics.clone());
        let grade_completion_mods =
            grade_completion_mods(&score.mods, score.grade, score.total_hits(), map);

        let (combo, title) = if score.mode == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}] [{}★]",
                KeyFormatter::new(&score.mods, map),
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
                round(*stars),
            );

            (combo, title)
        } else {
            let combo = ComboFormatter::new(score.max_combo, Some(*max_combo)).to_string();

            let title = format!(
                "{} - {} [{}] [{}★]",
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
                round(*stars),
            );

            (combo, title)
        };

        let footer = FooterBuilder::new(map.footer_text())
            .icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        let description = personal_idx
            .map(|idx| format!("__**Personal Best #{}**__", idx + 1))
            .unwrap_or_default();

        let name = format!(
            "{grade_completion_mods}\t{score}\t({acc}%)\t{ago}",
            score = WithComma::new(score.score),
            acc = round(score.accuracy),
            ago = HowLongAgoDynamic::new(&score.ended_at),
        );

        let pp = match self.minimized_pp {
            MinimizedPp::IfFc => {
                let mut result = String::with_capacity(17);
                result.push_str("**");
                let _ = write!(result, "{:.2}", score.pp);

                let _ = if let Some(ref if_fc) = if_fc {
                    write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp)
                } else {
                    write!(result, "**/{:.2}PP", max_pp.max(score.pp))
                };

                result
            }
            MinimizedPp::MaxPp => PpFormatter::new(Some(score.pp), Some(*max_pp)).to_string(),
        };

        let value = format!("{pp} [ {combo} ] {hits}");

        let url = format!("{OSU_BASE}b/{}", map.map_id());

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .fields(fields![name, value, false])
            .footer(footer)
            .thumbnail(map.thumbnail())
            .title(title)
            .url(url);

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}

impl IActiveMessage for TopPagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        match self.list_size {
            ListSize::Condensed => self.build_condensed().boxed(),
            ListSize::Detailed => self.build_detailed().boxed(),
            ListSize::Single => Box::pin(self.build_single(ctx)),
        }
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let defer = matches!(self.list_size, ListSize::Single);

        handle_pagination_component(ctx, component, self.msg_owner, defer, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        let defer = matches!(self.list_size, ListSize::Single);

        handle_pagination_modal(ctx, modal, self.msg_owner, defer, &mut self.pages)
    }
}

pub struct TopPaginationBuilder {
    user: Option<RedisData<User>>,
    mode: Option<GameMode>,
    entries: Option<Box<[TopEntry]>>,
    sort_by: Option<TopScoreOrder>,
    farm: Option<Farm>,
    list_size: Option<ListSize>,
    minimized_pp: Option<MinimizedPp>,
    content: Option<Box<str>>,
    msg_owner: Option<Id<UserMarker>>,
}

impl TopPaginationBuilder {
    pub fn build(&mut self) -> TopPagination {
        let user = self.user.take().expect("missing user");
        let mode = self.mode.expect("missing mode");
        let entries = self.entries.take().expect("missing entries");
        let sort_by = self.sort_by.expect("missing sort_by");
        let farm = self.farm.take().expect("missing farm");
        let list_size = self.list_size.expect("missing list_size");
        let minimized_pp = self.minimized_pp.expect("missing minimized_pp");
        let content = self.content.take().expect("missing content");
        let msg_owner = self.msg_owner.expect("missing msg_owner");

        let pages = match list_size {
            ListSize::Condensed => Pages::new(10, entries.len()),
            ListSize::Detailed => Pages::new(5, entries.len()),
            ListSize::Single => Pages::new(1, entries.len()),
        };

        TopPagination {
            user,
            mode,
            entries,
            sort_by,
            farm,
            list_size,
            minimized_pp,
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

    pub fn entries(&mut self, entries: Box<[TopEntry]>) -> &mut Self {
        self.entries = Some(entries);

        self
    }

    pub fn sort_by(&mut self, sort_by: TopScoreOrder) -> &mut Self {
        self.sort_by = Some(sort_by);

        self
    }

    pub fn farm(&mut self, farm: Farm) -> &mut Self {
        self.farm = Some(farm);

        self
    }

    pub fn list_size(&mut self, list_size: ListSize) -> &mut Self {
        self.list_size = Some(list_size);

        self
    }

    pub fn minimized_pp(&mut self, minimized_pp: MinimizedPp) -> &mut Self {
        self.minimized_pp = Some(minimized_pp);

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

        write!(
            f,
            "{miss}{emote} ",
            miss = self.0,
            emote = Emote::Miss.text()
        )
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
    entry: &'a TopEntry,
    ranked_date: Option<OffsetDateTime>,
    farm: &'a Farm,
    condensed: bool,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(
        sort_by: TopScoreOrder,
        entry: &'a TopEntry,
        ranked_date: Option<OffsetDateTime>,
        farm: &'a Farm,
        condensed: bool,
    ) -> Self {
        Self {
            sort_by,
            entry,
            ranked_date,
            farm,
            condensed,
        }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            TopScoreOrder::Farm => {
                let mapset_id = self.entry.map.mapset_id();

                let count = self
                    .farm
                    .get(&mapset_id)
                    .map_or(0, |(entry, _)| entry.count);

                write!(f, "`{}`", WithComma::new(count))
            }
            TopScoreOrder::Bpm => {
                let clock_rate = self.entry.score.mods.clock_rate().unwrap_or(1.0);

                write!(f, "`{}bpm`", round(self.entry.map.bpm() * clock_rate))
            }
            TopScoreOrder::Length => {
                let clock_rate = self.entry.score.mods.clock_rate().unwrap_or(1.0);

                let secs = (self.entry.map.seconds_drain() as f32 / clock_rate) as u32;

                write!(f, "`{}:{:0>2}`", secs / 60, secs % 60)
            }
            TopScoreOrder::RankedDate => match self.ranked_date {
                Some(date) => write!(f, "<t:{}:d>", date.unix_timestamp()),
                None => Ok(()),
            },
            TopScoreOrder::Score if self.condensed && self.entry.map.mode() != GameMode::Mania => {
                let score = self.entry.score.score;

                if score > 1_000_000_000 {
                    write!(f, "`{:.2}bn`", score as f32 / 1_000_000_000.0)
                } else if score > 1_000_000 {
                    write!(f, "`{:.2}m`", score as f32 / 1_000_000.0)
                } else {
                    write!(f, "`{}`", WithComma::new(score))
                }
            }
            _ => HowLongAgoDynamic::new(&self.entry.score.ended_at).fmt(f),
        }
    }
}
