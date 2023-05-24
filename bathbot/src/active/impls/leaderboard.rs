use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::ScraperScore;
use bathbot_util::{
    constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    numbers::WithComma,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::{BeatmapExt, DifficultyAttributes, ScoreState};
use rosu_v2::prelude::{CountryCode, GameMode};
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::LeaderboardUserScore,
    core::Context,
    embeds::PpFormatter,
    manager::{OsuMap, PpManager},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::grade_emote,
        Emote,
    },
};

type AttrMap = HashMap<u32, (DifficultyAttributes, f32), IntHasher>;

#[derive(PaginationBuilder)]
pub struct LeaderboardPagination {
    map: OsuMap,
    #[pagination(per_page = 10)]
    scores: Box<[ScraperScore]>,
    stars: f32,
    max_combo: u32,
    attr_map: AttrMap,
    author_data: Option<LeaderboardUserScore>,
    first_place_icon: Option<String>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for LeaderboardPagination {
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

impl LeaderboardPagination {
    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let start_idx = self.pages.index();
        let end_idx = self.scores.len().min(start_idx + self.pages.per_page());

        let mut author_text = String::with_capacity(32);

        if self.map.mode() == GameMode::Mania {
            let _ = write!(author_text, "[{}K] ", self.map.cs() as u32);
        }

        let _ = write!(
            author_text,
            "{artist} - {title} [{version}] [{stars:.2}★]",
            artist = self.map.artist().cow_escape_markdown(),
            title = self.map.title().cow_escape_markdown(),
            version = self.map.version().cow_escape_markdown(),
            stars = self.stars,
        );

        let author_name = self
            .author_data
            .as_ref()
            .map(|score| score.username.as_str());

        let mut description = String::with_capacity(1024);

        for (score, i) in self.scores[start_idx..end_idx].iter().zip(start_idx + 1..) {
            let found_author = author_name == Some(score.username.as_str());

            let fmt_fut = ScoreFormatter::new(
                i,
                score,
                found_author,
                &ctx,
                &mut self.attr_map,
                &self.map,
                self.max_combo,
            );

            let _ = write!(description, "{}", fmt_fut.await);
        }

        if let Some(score) = self
            .author_data
            .as_ref()
            .filter(|score| !(start_idx + 1..end_idx + 1).contains(&score.pos))
        {
            let scraper_score = ScraperScore {
                id: 0,
                user_id: score.user_id,
                username: score.username.clone(),
                country_code: CountryCode::new(),
                accuracy: score.accuracy,
                mode: self.map.mode(), // TODO: fix when mode selection available
                mods: score.mods.clone(),
                score: score.score,
                max_combo: score.combo,
                pp: score.pp,
                grade: score.grade,
                date: score.ended_at,
                replay: false,
                count50: score.statistics.count_50,
                count100: score.statistics.count_100,
                count300: score.statistics.count_300,
                count_geki: score.statistics.count_geki,
                count_katu: score.statistics.count_katu,
                count_miss: score.statistics.count_miss,
            };

            let _ = writeln!(description, "\n__**<@{}>'s score:**__", score.discord_id);

            let fmt_fut = ScoreFormatter::new(
                score.pos,
                &scraper_score,
                false,
                &ctx,
                &mut self.attr_map,
                &self.map,
                self.max_combo,
            );

            let _ = write!(description, "{}", fmt_fut.await);
        }

        let mut author =
            AuthorBuilder::new(author_text).url(format!("{OSU_BASE}b/{}", self.map.map_id()));

        if let Some(ref author_icon) = self.first_place_icon {
            author = author.icon_url(author_icon.to_owned());
        }

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} mapset of {creator}",
            status = self.map.status(),
            creator = self.map.creator(),
        );

        let footer_icon = format!(
            "{AVATAR_URL}{creator_id}",
            creator_id = self.map.creator_id()
        );
        let footer = FooterBuilder::new(footer_text).icon_url(footer_icon);

        let thumbnail = format!("{MAP_THUMB_URL}{}l.jpg", self.map.mapset_id());

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .thumbnail(thumbnail);

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}

struct ComboFormatter<'a> {
    score: &'a ScraperScore,
    max_combo: u32,
    mode: GameMode,
}

impl<'a> ComboFormatter<'a> {
    fn new(score: &'a ScraperScore, max_combo: u32, mode: GameMode) -> Self {
        Self {
            score,
            max_combo,
            mode,
        }
    }
}

impl<'a> Display for ComboFormatter<'a> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "**{}x**", self.score.max_combo)?;

        if self.mode == GameMode::Mania {
            let mut ratio = self.score.count_geki as f32;

            if self.score.count300 > 0 {
                ratio /= self.score.count300 as f32
            }

            write!(f, " / {ratio:.2}")
        } else {
            write!(f, "/{}x", self.max_combo)
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

struct ScoreFormatter<'a> {
    i: usize,
    score: &'a ScraperScore,
    pp: PpFormatter,
    combo: ComboFormatter<'a>,
    found_author: bool,
}

impl<'a> ScoreFormatter<'a> {
    async fn new(
        i: usize,
        score: &'a ScraperScore,
        found_author: bool,
        ctx: &Context,
        attr_map: &mut AttrMap,
        map: &OsuMap,
        max_combo: u32,
    ) -> ScoreFormatter<'a> {
        let mods = score.mods.bits();

        let pp = match attr_map.entry(mods) {
            Entry::Occupied(entry) => {
                let (attrs, max_pp) = entry.get();

                let state = ScoreState {
                    max_combo: score.max_combo as usize,
                    n_geki: score.count_geki as usize,
                    n_katu: score.count_katu as usize,
                    n300: score.count300 as usize,
                    n100: score.count100 as usize,
                    n50: score.count50 as usize,
                    n_misses: score.count_miss as usize,
                };

                let pp = map
                    .pp_map
                    .pp()
                    .attributes(attrs.to_owned())
                    .mode(PpManager::mode_conversion(score.mode))
                    .mods(mods)
                    .state(state)
                    .calculate()
                    .pp() as f32;

                PpFormatter::new(Some(pp), Some(*max_pp))
            }
            Entry::Vacant(entry) => {
                let mut calc = ctx.pp(map).mode(score.mode).mods(mods);
                let attrs = calc.performance().await;
                let max_pp = attrs.pp() as f32;
                let pp = calc.score(score).performance().await.pp() as f32;
                entry.insert((attrs.into(), max_pp));

                PpFormatter::new(Some(pp), Some(max_pp))
            }
        };

        let combo = ComboFormatter::new(score, max_combo, map.mode());

        Self {
            i,
            score,
            pp,
            combo,
            found_author,
        }
    }
}

impl Display for ScoreFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(
            f,
            "**#{i}** {underline}**[{username}]({OSU_BASE}users/{user_id})**{underline}: {score} [ {combo} ] **+{mods}**\n\
            {grade} {pp} • {acc:.2}% • {miss}{ago}",
            i = self.i,
            underline = if self.found_author { "__" } else { "" },
            username = self.score.username.cow_escape_markdown(),
            user_id = self.score.user_id,
            grade = grade_emote(self.score.grade),
            score = WithComma::new(self.score.score),
            combo = self.combo,
            mods = self.score.mods,
            pp = self.pp,
            acc = self.score.accuracy,
            miss = MissFormat(self.score.count_miss),
            ago = HowLongAgoDynamic::new(&self.score.date),
        )
    }
}
