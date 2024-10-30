use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_cache::value::CachedArchive;
use bathbot_macros::PaginationBuilder;
use bathbot_model::{
    rkyv_util::time::DateRkyv, ArchivedOsuStatsBestScore, ArchivedOsuStatsBestScores,
};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::{HowLongAgoDynamic, DATE_FORMAT},
    numbers::{round, WithComma},
    AuthorBuilder, EmbedBuilder, FooterBuilder, ModsFormatter,
};
use eyre::Result;
use futures::future::BoxFuture;
use rkyv::rancor::{Failure, Panic, ResultExt};
use rosu_v2::{
    model::Grade,
    prelude::{GameMode, GameMods},
};
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
    commands::osu::OsuStatsBestSort,
    core::BotConfig,
    embeds::ComboFormatter,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct OsuStatsBestPagination {
    #[pagination(per_page = 10, len = "scores.scores.len()")]
    scores: CachedArchive<ArchivedOsuStatsBestScores>,
    mode: GameMode,
    sort: OsuStatsBestSort,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for OsuStatsBestPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;

        let start_date = DateRkyv::try_deserialize(self.scores.start_date).unwrap();
        let end_date = DateRkyv::try_deserialize(self.scores.end_date).unwrap();
        let scores = &self.scores.scores;

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
            // TODO: make ModsFormatter work with archived mods?
            let mods = rkyv::api::deserialize_using::<GameMods, _, Panic>(&score.mods, &mut ())
                .always_ok();

            let _ = writeln!(
                description,
                "**#{i} [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id}) +{mods}**\n\
                by __[{user}]({OSU_BASE}u/{user_id})__ {grade} **{pp}pp** \
                • {acc}% • [ {combo} ] {appendix}",
                artist = score.map.artist,
                title = score.map.title,
                version = score.map.version,
                map_id = score.map.map_id,
                mods = ModsFormatter::new(&mods),
                user = score.user.username,
                user_id = score.user.user_id,
                grade = config.grade(Grade::from(score.grade)),
                pp = round(score.pp.to_native()),
                acc = round(score.accuracy.to_native()),
                combo = ComboFormatter::new(
                    score.max_combo.to_native(),
                    Some(score.map.max_combo.to_native())
                ),
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
    score: &'s ArchivedOsuStatsBestScore,
    sort: OsuStatsBestSort,
}

impl<'s> OrderAppendix<'s> {
    fn new(score: &'s ArchivedOsuStatsBestScore, sort: OsuStatsBestSort) -> Self {
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
                write!(f, "`{}`", WithComma::new(self.score.score.to_native()))
            }
            OsuStatsBestSort::Accuracy
            | OsuStatsBestSort::Combo
            | OsuStatsBestSort::Date
            | OsuStatsBestSort::Pp => {
                let ended_at = self
                    .score
                    .ended_at
                    .try_deserialize::<Failure>()
                    .ok()
                    .unwrap_or_else(OffsetDateTime::now_utc);

                Display::fmt(&HowLongAgoDynamic::new(&ended_at), f)
            }
        }
    }
}
