use std::{cmp::Ordering, fmt::Write};

use bathbot_model::{
    rkyv_util::time::ArchivedDateTime,
    rosu_v2::user::{ArchivedUser, ArchivedUserHighestRank},
};
use bathbot_util::{AuthorBuilder, constants::OSU_BASE, numbers::WithComma, osu::flag_url};
use rkyv::{munge::munge, niche::niched_option::NichedOption};

use crate::manager::redis::osu::CachedUser;

pub trait CachedUserExt {
    fn author_builder(&self, with_rank_change: bool) -> AuthorBuilder;
    fn rank_change_since_30_days(&self) -> Option<i32>;
    fn update(&mut self, user: rosu_v2::model::user::User);
}

impl CachedUserExt for CachedUser {
    fn author_builder(&self, with_rank_change: bool) -> AuthorBuilder {
        let stats = self.statistics.as_ref().expect("missing stats");
        let country_code = self.country_code.as_str();

        let mut text = format!(
            "{name}: {pp}pp (#{global}",
            name = self.username,
            pp = WithComma::new(stats.pp.to_native()),
            global = WithComma::new(stats.global_rank.to_native()),
        );

        // Arguably showing the value *always* is a little much so it's
        // probably best we only show it in some cases.
        if with_rank_change {
            if let Some(rank_change) = self.rank_change_since_30_days() {
                // Alternatively '🡩' and '🡫' or '⭜' and '⭝' but they don't
                // show properly on mobile and/or linux :(
                match rank_change.cmp(&0) {
                    Ordering::Greater => {
                        let _ = write!(text, " ⬇{rank_change}");
                    }
                    Ordering::Less => {
                        let _ = write!(text, " ⬆{}", -rank_change);
                    }
                    Ordering::Equal => {}
                }
            }
        }

        let _ = write!(
            text,
            " {country_code}{national})",
            national = stats.country_rank,
        );

        let url = format!("{OSU_BASE}users/{}/{}", self.user_id, self.mode);
        let icon = flag_url(country_code);

        AuthorBuilder::new(text).url(url).icon_url(icon)
    }

    fn rank_change_since_30_days(&self) -> Option<i32> {
        let prev = self
            .rank_history
            .as_ref()
            .get(self.rank_history.len().checked_sub(30)?)?
            .to_native() as i32;

        let curr = self.rank_history.last()?.to_native() as i32;

        (curr > 0 && prev > 0).then_some(curr - prev)
    }

    fn update(&mut self, user: rosu_v2::model::user::User) {
        self.mutate(|seal| {
            munge!(let ArchivedUser {
                last_visit: last_visit_seal,
                highest_rank: highest_rank_seal,
                follower_count: follower_count_seal,
                graveyard_mapset_count: graveyard_mapset_count_seal,
                guest_mapset_count: guest_mapset_count_seal,
                loved_mapset_count: loved_mapset_count_seal,
                ranked_mapset_count: ranked_mapset_count_seal,
                scores_first_count: scores_first_count_seal,
                pending_mapset_count: pending_mapset_count_seal,
                statistics: statistics_seal,
                avatar_url: _,
                country_code: _,
                join_date: _,
                kudosu: _,
                mode: _,
                user_id: _,
                username: _,
                badges: _,
                mapping_follower_count: _,
                monthly_playcounts: _,
                rank_history: _,
                replays_watched_counts: _,
                medals: _,
                daily_challenge: _,
                team: _,
            } = seal);

            if let Some(last_visit) = user.last_visit {
                if let Some(last_visit_seal) = NichedOption::as_seal(last_visit_seal) {
                    *last_visit_seal.unseal() = ArchivedDateTime::new(last_visit);
                }
            }

            if let Some(stats) = user.statistics {
                if let Some(stats_seal) = NichedOption::as_seal(statistics_seal) {
                    // SAFETY: We neither move fields nor write uninitialized bytes
                    unsafe { *stats_seal.unseal_unchecked() = stats.into() };
                }
            }

            if let Some(highest_rank) = user.highest_rank {
                if let Some(highest_rank_seal) = NichedOption::as_seal(highest_rank_seal) {
                    // SAFETY: We neither move fields nor write uninitialized bytes
                    unsafe {
                        *highest_rank_seal.unseal_unchecked() = ArchivedUserHighestRank {
                            rank: highest_rank.rank.into(),
                            updated_at: ArchivedDateTime::new(highest_rank.updated_at),
                        }
                    };
                }
            }

            macro_rules! update_pod {
                ( $field:ident: $seal:ident ) => {
                    if let Some($field) = user.$field {
                        *$seal.unseal() = $field.into();
                    }
                };
            }

            update_pod!(follower_count: follower_count_seal);
            update_pod!(graveyard_mapset_count: graveyard_mapset_count_seal);
            update_pod!(guest_mapset_count: guest_mapset_count_seal);
            update_pod!(loved_mapset_count: loved_mapset_count_seal);
            update_pod!(ranked_mapset_count: ranked_mapset_count_seal);
            update_pod!(scores_first_count: scores_first_count_seal);
            update_pod!(pending_mapset_count: pending_mapset_count_seal);
        });
    }
}
