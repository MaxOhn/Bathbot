use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::WithComma, CowUtils, EmbedBuilder,
    FooterBuilder, ModsFormatter, ScoreExt,
};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::NochokeEntry,
    manager::redis::osu::CachedUser,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeFormatter,
        CachedUserExt, Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct NoChokePagination {
    user: CachedUser,
    #[pagination(per_page = 5)]
    entries: Box<[NochokeEntry]>,
    unchoked_pp: f32,
    rank: Option<u32>,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for NoChokePagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let pp_raw = self
            .user
            .statistics
            .as_ref()
            .expect("missing stats")
            .pp
            .to_native();

        let pp_diff = (100.0 * (self.unchoked_pp - pp_raw)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for entry in entries {
            let NochokeEntry {
                original_idx,
                original_score,
                map,
                max_pp,
                stars,
                unchoked,
                max_combo,
            } = entry;

            let misses = match unchoked {
                Some(_) => MissFormat::Misses(original_score.statistics.miss),
                None => match original_score.statistics.miss {
                    0 => MissFormat::None,
                    _ => MissFormat::Skipped,
                },
            };

            let _ = writeln!(
                description,
                "**#{idx} [{title} [{version}]]({OSU_BASE}b/{id}) +{mods}** [{stars:.2}★]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP • {old_acc:.2} → **{new_acc:.2}%**\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ]{misses} • {score_timestamp}",
                idx = original_idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(&original_score.mods),
                grade = GradeFormatter::new(entry.unchoked_grade(), Some(entry.original_score.score_id), entry.original_score.is_legacy()),
                old_pp = original_score.pp,
                new_pp = entry.unchoked_pp(),
                old_acc = original_score.accuracy,
                new_acc = entry.unchoked_accuracy(),
                old_combo = original_score.max_combo,
                new_combo = entry.unchoked_max_combo(),
                score_timestamp = HowLongAgoDynamic::new(&original_score.ended_at)
            );
        }

        let title = format!(
            "Total pp: {pp_raw} → **{unchoked_pp}pp** (+{pp_diff})",
            unchoked_pp = self.unchoked_pp
        );

        let page = pages.curr_page();
        let pages = pages.last_page();
        let mut footer_text = format!("Page {page}/{pages}");

        if let Some(rank) = self.rank {
            let _ = write!(
                footer_text,
                " • The current rank for {pp}pp is approx. #{rank}",
                pp = WithComma::new(self.unchoked_pp),
                rank = WithComma::new(rank)
            );
        }

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder(false))
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url.as_ref())
            .title(title);

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

enum MissFormat {
    Misses(u32),
    Skipped,
    None,
}

impl Display for MissFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            MissFormat::Misses(count) => {
                write!(f, " • *Removed {count}{emote}*", emote = Emote::Miss)
            }
            MissFormat::Skipped => f.write_str(" • *Skipped :track_next:*"),
            MissFormat::None => Ok(()),
        }
    }
}
