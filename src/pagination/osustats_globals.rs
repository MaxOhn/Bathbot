use std::collections::BTreeMap;

use command_macros::pagination;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::{GameMode, Grade, ScoreStatistics};
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::OsuStatsEntry,
    custom_client::OsuStatsParams,
    embeds::{EmbedData, OsuStatsGlobalsEmbed},
    manager::redis::{osu::User, RedisData},
    util::osu::ScoreSlim,
    Context,
};

use super::Pages;

#[pagination(per_page = 5, total = "total")]
pub struct OsuStatsGlobalsPagination {
    user: RedisData<User>,
    entries: BTreeMap<usize, OsuStatsEntry>,
    total: usize,
    params: OsuStatsParams,
}

impl OsuStatsGlobalsPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let entries = self
            .entries
            .range(pages.index..pages.index + pages.per_page);
        let count = entries.count();

        if count < pages.per_page && self.total - pages.index > count {
            let osustats_page = (pages.index / 24) + 1;
            self.params.page = osustats_page;

            let (scores, _) = ctx
                .client()
                .get_global_scores(&self.params)
                .await
                .wrap_err("failed to get global scores")?;

            let maps_id_checksum = scores
                .iter()
                .map(|score| (score.map.map_id as i32, None))
                .collect();

            let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;
            let mode = self.params.mode;

            for (score, i) in scores.into_iter().zip((osustats_page - 1) * 24..) {
                let map_opt = maps.remove(&score.map.map_id);
                let Some(map) = map_opt else { continue };

                let mut calc = ctx.pp(&map).mods(score.mods).mode(mode);
                let attrs = calc.performance().await;

                let pp = match score.pp {
                    Some(pp) => pp,
                    None => calc.score(&score).performance().await.pp() as f32,
                };

                let max_pp =
                    if score.grade.eq_letter(Grade::X) && mode != GameMode::Mania && pp > 0.0 {
                        pp
                    } else {
                        attrs.pp() as f32
                    };

                let rank = score.position;

                let score = ScoreSlim {
                    accuracy: score.accuracy,
                    ended_at: score.ended_at,
                    grade: score.grade,
                    max_combo: score.max_combo,
                    mode,
                    mods: score.mods,
                    pp,
                    score: score.score,
                    score_id: None,
                    statistics: ScoreStatistics {
                        count_geki: score.count_geki,
                        count_300: score.count300,
                        count_katu: score.count_katu,
                        count_100: score.count100,
                        count_50: score.count50,
                        count_miss: score.count_miss,
                    },
                };

                let entry = OsuStatsEntry {
                    score,
                    map,
                    rank,
                    max_pp,
                    stars: attrs.stars() as f32,
                };

                self.entries.insert(i, entry);
            }
        }

        let embed = OsuStatsGlobalsEmbed::new(&self.user, &self.entries, self.total, pages);

        Ok(embed.build())
    }
}
