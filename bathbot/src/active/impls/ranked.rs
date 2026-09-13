use std::fmt::Write;

use bathbot_macros::PaginationBuilder;
use bathbot_model::rosu_v2::matchmaking::{MatchmakingResultRkyv, MatchmakingUserStatsRkyv};
use bathbot_util::{CowUtils, EmbedBuilder, FooterBuilder, fields, numbers::WithComma};
use eyre::Result;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    manager::redis::osu::CachedUser,
    util::{
        CachedUserExt,
        interaction::{InteractionComponent, InteractionModal},
    },
};

/// Build the embed of the ranked play statistics of a single pool.
pub fn build_embed(user: &CachedUser, stats: &MatchmakingUserStatsRkyv) -> EmbedBuilder {
    let pool = stats
        .pool
        .as_ref()
        .expect("ranked play stats without a pool");

    let rating = if stats.is_rating_provisional {
        format!("{} (provisional)", WithComma::new(stats.rating))
    } else {
        WithComma::new(stats.rating).to_string()
    };

    // `rank_percent` is the fraction of the pool's players ranked at or above
    // the user. Even #1 does not beat *all* players (i.e. itself) so "100.000%"
    // is never rendered.
    let beats = format!("{:.3}", (1.0 - stats.rank_percent) * 100.0);

    let wins = if stats.plays > 0 {
        format!(
            "{} ({:.1}%)",
            WithComma::new(stats.first_placements),
            stats.first_placements as f64 / stats.plays as f64 * 100.0
        )
    } else {
        WithComma::new(stats.first_placements).to_string()
    };

    let mut pool_name = pool.name.cow_escape_markdown().into_owned();

    if !pool.active {
        pool_name.push_str(" (*inactive*)");
    }

    let mut description = String::with_capacity(128);

    let _ = writeln!(description, "**Pool:** {pool_name}");
    let _ = writeln!(description, "**Rating:** {rating}");
    let _ = writeln!(
        description,
        "**Rank:** #{rank} - beats {beats}% of players",
        rank = WithComma::new(stats.rank),
    );

    let mut fields = fields![
        "🎮 Plays", WithComma::new(stats.plays).to_string(), true;
        "🏆 Wins", wins, true;
    ];

    if !stats.recent_history.is_empty() {
        const MAX_COUNT: usize = 15;

        let recent = stats
            .recent_history
            .iter()
            .rev()
            .take(MAX_COUNT)
            .map(|entry| match entry.result {
                MatchmakingResultRkyv::Win => "🟢",
                MatchmakingResultRkyv::Loss => "🔴",
                MatchmakingResultRkyv::Draw => "⚪",
            })
            .collect::<String>();

        let recent_len = stats.recent_history.len().min(MAX_COUNT);

        let wins = stats
            .recent_history
            .iter()
            .take(MAX_COUNT)
            .filter(|entry| entry.result == MatchmakingResultRkyv::Win)
            .count();

        let recent = format!("{recent} - {wins}/{recent_len} wins");

        fields!(fields { "Recent matches", recent, false });
    }

    EmbedBuilder::new()
        .author(user.author_builder(false))
        .title("Ranked Play Statistics")
        .description(description)
        .fields(fields)
        .thumbnail(user.avatar_url.as_ref())
}

#[derive(PaginationBuilder)]
pub struct RankedPagination {
    user: CachedUser,
    #[pagination(per_page = 1)]
    pools: Box<[MatchmakingUserStatsRkyv]>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for RankedPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let pages = &self.pages;
        let stats = &self.pools[pages.index()];

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        let embed = build_embed(&self.user, stats).footer(FooterBuilder::new(footer_text));

        Ok(BuildPage::new(embed, false))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages).await
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages).await
    }
}
