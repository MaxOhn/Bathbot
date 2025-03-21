use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{OsuStatsBestScore, OsuStatsBestScores};
use bathbot_util::{
    AuthorBuilder, EmbedBuilder, FooterBuilder, ModsFormatter,
    constants::OSU_BASE,
    datetime::{DATE_FORMAT, HowLongAgoDynamic},
    numbers::{WithComma, round},
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::osu::OsuStatsBestSort,
    core::BotConfig,
    embeds::ComboFormatter,
    util::{
        Emote,
        interaction::{InteractionComponent, InteractionModal},
    },
};

#[derive(PaginationBuilder)]
pub struct OsuStatsBestPagination {
    #[pagination(per_page = 10, len = "scores.scores.len()")]
    scores: OsuStatsBestScores,
    mode: GameMode,
    sort: OsuStatsBestSort,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for OsuStatsBestPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;

        let OsuStatsBestScores {
            start_date,
            end_date,
            ref scores,
        } = self.scores;

        let author_text = format!(
            "Top {mode} scores between {start} and {end}:",
            mode = match self.mode {
                GameMode::Osu => "osu!",
                GameMode::Taiko => "taiko",
                GameMode::Catch => "ctb",
                GameMode::Mania => "mania",
            },
            start = start_date.format(DATE_FORMAT).unwrap(),
            end = end_date.format(DATE_FORMAT).unwrap(),
        );

        let author = AuthorBuilder::new(author_text).url("https://osustats.ppy.sh/");

        let footer_text = format!(
            "Page {page}/{pages} • Sorted by {sort}",
            page = pages.curr_page(),
            pages = pages.last_page(),
            sort = match self.sort {
                OsuStatsBestSort::Accuracy => "accuracy",
                OsuStatsBestSort::Combo => "combo",
                OsuStatsBestSort::Date => "date",
                OsuStatsBestSort::LeaderboardPosition => "map leaderboard",
                OsuStatsBestSort::Misses => "misses",
                OsuStatsBestSort::Pp => "pp",
                OsuStatsBestSort::Score => "score",
            }
        );

        let footer = FooterBuilder::new(footer_text);

        let idx = pages.index();
        let scores = &scores[idx..scores.len().min(idx + pages.per_page())];

        let config = BotConfig::get();
        let mut description = String::with_capacity(1024);

        for (score, i) in scores.iter().zip(idx + 1..) {
            let _ = writeln!(
                description,
                "**#{i} [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}**\n\
                by __[{user}]({OSU_BASE}u/{user_id})__ {grade} **{pp}pp** \
                • {acc}% • [ {combo} ] {appendix}",
                artist = score.map.artist,
                title = score.map.title,
                version = score.map.version,
                map_id = score.map.map_id,
                mods = ModsFormatter::new(&score.mods),
                user = score.user.username,
                user_id = score.user.user_id,
                grade = config.grade(score.grade.into()),
                pp = round(score.pp),
                acc = round(score.accuracy),
                combo = ComboFormatter::new(score.max_combo, Some(score.map.max_combo)),
                appendix = OrderAppendix::new(score, self.sort),
            );
        }

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer);

        BuildPage::new(embed, false).boxed()
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
    score: &'s OsuStatsBestScore,
    sort: OsuStatsBestSort,
}

impl<'s> OrderAppendix<'s> {
    fn new(score: &'s OsuStatsBestScore, sort: OsuStatsBestSort) -> Self {
        Self { score, sort }
    }
}

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort {
            OsuStatsBestSort::LeaderboardPosition => write!(f, "`#{}`", self.score.position),
            OsuStatsBestSort::Misses => write!(
                f,
                "{miss}{emote}",
                miss = self.score.count_miss,
                emote = Emote::Miss
            ),
            OsuStatsBestSort::Score => {
                write!(f, "`{}`", WithComma::new(self.score.score))
            }
            OsuStatsBestSort::Accuracy
            | OsuStatsBestSort::Combo
            | OsuStatsBestSort::Date
            | OsuStatsBestSort::Pp => {
                Display::fmt(&HowLongAgoDynamic::new(&self.score.ended_at), f)
            }
        }
    }
}
