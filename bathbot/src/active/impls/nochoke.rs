use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, CowUtils, EmbedBuilder, FooterBuilder,
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
    core::Context,
    embeds::ModsFormatter,
    manager::redis::RedisData,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::grade_emote,
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct NoChokePagination {
    user: RedisData<User>,
    #[pagination(per_page = 5)]
    entries: Box<[NochokeEntry]>,
    unchoked_pp: f32,
    rank: Option<u32>,
    content: String,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for NoChokePagination {
    fn build_page<'a>(&'a mut self, _: Arc<Context>) -> BoxFuture<'a, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let pp_raw = self.user.stats().pp();
        let pp_diff = (100.0 * (self.unchoked_pp - pp_raw)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for entry in entries {
            let NochokeEntry {
                original_idx,
                original_score,
                map,
                max_pp,
                stars,
                unchoked: _,
                max_combo,
            } = entry;

            let unchoked_stats = entry.unchoked_statistics();

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP • ({old_acc:.2} → **{new_acc:.2}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ]{misses}",
                idx = original_idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(&original_score.mods),
                grade = grade_emote(entry.unchoked_grade()),
                old_pp = original_score.pp,
                new_pp = entry.unchoked_pp(),
                old_acc = original_score.accuracy,
                new_acc = entry.unchoked_accuracy(),
                old_combo = original_score.max_combo,
                new_combo = entry.unchoked_max_combo(),
                misses = MissFormat(original_score.statistics.count_miss - unchoked_stats.count_miss),
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
            .author(self.user.author_builder())
            .description(description)
            .footer(FooterBuilder::new(footer_text))
            .thumbnail(self.user.avatar_url())
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
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(ctx, component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, false, &mut self.pages)
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
            " • *Removed {miss}{emote}*",
            miss = self.0,
            emote = Emote::Miss.text()
        )
    }
}
