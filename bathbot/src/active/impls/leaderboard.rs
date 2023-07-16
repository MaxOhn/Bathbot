use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    numbers::WithComma,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_pp::{BeatmapExt, DifficultyAttributes, ScoreState};
use rosu_v2::prelude::{GameMode, Grade, Score, UserCompact};
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
    scores: Box<[Score]>,
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
            let found_author = score
                .user
                .as_ref()
                .is_some_and(|user| Some(user.username.as_str()) == author_name);

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
            let score_ = Score {
                accuracy: score.accuracy,
                ended_at: score.ended_at,
                passed: score.grade != Grade::F,
                grade: score.grade,
                map_id: self.map.map_id(),
                max_combo: score.combo,
                map: None,
                mapset: None,
                mode: self.map.mode(), // TODO: fix when mode selection available
                mods: score.mods.clone(),
                perfect: false,
                pp: score.pp,
                rank_country: None,
                rank_global: None,
                replay: None,
                score: score.score,
                score_id: None,
                statistics: score.statistics.clone(),
                user: Some(UserCompact {
                    avatar_url: Default::default(),
                    country_code: Default::default(),
                    default_group: Default::default(),
                    is_active: Default::default(),
                    is_bot: Default::default(),
                    is_deleted: Default::default(),
                    is_online: Default::default(),
                    is_supporter: Default::default(),
                    last_visit: Default::default(),
                    pm_friends_only: Default::default(),
                    profile_color: Default::default(),
                    user_id: Default::default(),
                    username: score.username.clone(),
                    account_history: Default::default(),
                    badges: Default::default(),
                    beatmap_playcounts_count: Default::default(),
                    country: Default::default(),
                    cover: Default::default(),
                    favourite_mapset_count: Default::default(),
                    follower_count: Default::default(),
                    graveyard_mapset_count: Default::default(),
                    groups: Default::default(),
                    guest_mapset_count: Default::default(),
                    highest_rank: Default::default(),
                    is_admin: Default::default(),
                    is_bng: Default::default(),
                    is_full_bn: Default::default(),
                    is_gmt: Default::default(),
                    is_limited_bn: Default::default(),
                    is_moderator: Default::default(),
                    is_nat: Default::default(),
                    is_silenced: Default::default(),
                    loved_mapset_count: Default::default(),
                    medals: Default::default(),
                    monthly_playcounts: Default::default(),
                    page: Default::default(),
                    previous_usernames: Default::default(),
                    rank_history: Default::default(),
                    ranked_mapset_count: Default::default(),
                    replays_watched_counts: Default::default(),
                    scores_best_count: Default::default(),
                    scores_first_count: Default::default(),
                    scores_recent_count: Default::default(),
                    statistics: Default::default(),
                    support_level: Default::default(),
                    pending_mapset_count: Default::default(),
                }),
                user_id: score.user_id,
                weight: None,
            };

            let _ = writeln!(description, "\n__**<@{}>'s score:**__", score.discord_id);

            let fmt_fut = ScoreFormatter::new(
                score.pos,
                &score_,
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

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .thumbnail(self.map.thumbnail());

        Ok(BuildPage::new(embed, true).content(self.content.clone()))
    }
}

struct ComboFormatter<'a> {
    score: &'a Score,
    max_combo: u32,
    mode: GameMode,
}

impl<'a> ComboFormatter<'a> {
    fn new(score: &'a Score, max_combo: u32, mode: GameMode) -> Self {
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
            let mut ratio = self.score.statistics.count_geki as f32;

            if self.score.statistics.count_300 > 0 {
                ratio /= self.score.statistics.count_300 as f32
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

        write!(f, "{miss}{emote} ", miss = self.0, emote = Emote::Miss)
    }
}

struct ScoreFormatter<'a> {
    i: usize,
    score: &'a Score,
    pp: PpFormatter,
    combo: ComboFormatter<'a>,
    found_author: bool,
}

impl<'a> ScoreFormatter<'a> {
    async fn new(
        i: usize,
        score: &'a Score,
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
                    n_geki: score.statistics.count_geki as usize,
                    n_katu: score.statistics.count_katu as usize,
                    n300: score.statistics.count_300 as usize,
                    n100: score.statistics.count_100 as usize,
                    n50: score.statistics.count_50 as usize,
                    n_misses: score.statistics.count_miss as usize,
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
            username = self.score.user.as_ref().map_or("<unknown user>", |user| user.username.as_str()),
            user_id = self.score.user_id,
            grade = grade_emote(self.score.grade),
            score = WithComma::new(self.score.score),
            combo = self.combo,
            mods = self.score.mods,
            pp = self.pp,
            acc = self.score.accuracy,
            miss = MissFormat(self.score.statistics.count_miss),
            ago = HowLongAgoDynamic::new(&self.score.ended_at),
        )
    }
}
