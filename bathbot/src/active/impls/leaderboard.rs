use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::WithComma, AuthorBuilder, CowUtils,
    EmbedBuilder, FooterBuilder, ModsFormatter,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{LeaderboardScore, LeaderboardUserScore},
    embeds::PpFormatter,
    manager::OsuMap,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeFormatter,
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct LeaderboardPagination {
    map: OsuMap,
    #[pagination(per_page = 10)]
    scores: Box<[LeaderboardScore]>,
    stars: f32,
    max_combo: u32,
    author_data: Option<LeaderboardUserScore>,
    first_place_icon: Option<String>,
    score_data: ScoreData,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for LeaderboardPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page())
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, true, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, true, &mut self.pages)
    }
}

impl LeaderboardPagination {
    async fn async_build_page(&mut self) -> Result<BuildPage> {
        let start_idx = self.pages.index();
        let end_idx = self.scores.len().min(start_idx + self.pages.per_page());

        let mut author_text = String::with_capacity(32);

        if self.map.mode() == GameMode::Mania {
            let _ = write!(
                author_text,
                "[{}K] ",
                self.map.attributes().build().cs as u32
            );
        }

        let _ = write!(
            author_text,
            "{artist} - {title} [{version}] [{stars:.2}★]",
            artist = self.map.artist().cow_escape_markdown(),
            title = self.map.title().cow_escape_markdown(),
            version = self.map.version().cow_escape_markdown(),
            stars = self.stars,
        );

        let author_name = self.author_data.as_ref().map(|score| score.score.user_id);

        let mut description = String::with_capacity(1024);

        for score in self.scores[start_idx..end_idx].iter_mut() {
            let found_author = Some(score.user_id) == author_name;

            let fmt_fut = ScoreFormatter::new(
                score,
                found_author,
                &self.map,
                self.max_combo,
                self.score_data,
            );

            let _ = write!(description, "{}", fmt_fut.await);
        }

        if let Some(score) = self
            .author_data
            .as_mut()
            .filter(|score| !(start_idx + 1..=end_idx).contains(&score.score.pos))
        {
            let _ = writeln!(description, "\n__**<@{}>'s score:**__", score.discord_id);

            let fmt_fut = ScoreFormatter::new(
                &mut score.score,
                false,
                &self.map,
                self.max_combo,
                self.score_data,
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

        let footer_icon = Emote::from(self.map.mode()).url();
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
    score: &'a LeaderboardScore,
    max_combo: u32,
    mode: GameMode,
}

impl<'a> ComboFormatter<'a> {
    fn new(score: &'a LeaderboardScore, max_combo: u32, mode: GameMode) -> Self {
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
        write!(f, "**{}x**", self.score.combo)?;

        if self.mode == GameMode::Mania {
            let mut ratio = self.score.statistics.perfect as f32;

            if self.score.statistics.great > 0 {
                ratio /= self.score.statistics.great as f32
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
    score: &'a LeaderboardScore,
    pp: PpFormatter,
    combo: ComboFormatter<'a>,
    found_author: bool,
    score_data: ScoreData,
}

impl<'a> ScoreFormatter<'a> {
    async fn new(
        score: &'a mut LeaderboardScore,
        found_author: bool,
        map: &OsuMap,
        max_combo: u32,
        score_data: ScoreData,
    ) -> ScoreFormatter<'a> {
        let pps = score.pp(map).await;
        let pp = PpFormatter::new(Some(pps.pp), Some(pps.max));
        let combo = ComboFormatter::new(score, max_combo, map.mode());

        Self {
            score,
            pp,
            combo,
            found_author,
            score_data,
        }
    }
}

impl Display for ScoreFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let score = match self.score_data {
            ScoreData::Stable | ScoreData::Lazer => self.score.score,
            ScoreData::LazerWithClassicScoring if self.score.classic_score == 0 => self.score.score,
            ScoreData::LazerWithClassicScoring => self.score.classic_score,
        };

        writeln!(
            f,
            "**#{i}** {underline}**[{username}]({OSU_BASE}users/{user_id})**{underline}: {score} [ {combo} ] **+{mods}**\n\
            {grade} {pp} • {acc:.2}% • {miss}{ago}",
            i = self.score.pos,
            underline = if self.found_author { "__" } else { "" },
            username = self.score.username,
            user_id = self.score.user_id,
            grade = GradeFormatter::new(self.score.grade, Some(self.score.score_id), self.score.is_legacy),
            score = WithComma::new(score),
            combo = self.combo,
            mods = ModsFormatter::new(&self.score.mods),
            pp = self.pp,
            acc = self.score.accuracy,
            miss = MissFormat(self.score.statistics.miss),
            ago = HowLongAgoDynamic::new(&self.score.ended_at),
        )
    }
}
