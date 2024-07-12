use std::collections::HashMap;

use bathbot_model::{BgGameScore, HlGameScore, HlVersion};
use bathbot_psql::{
    model::games::{DbMapTagsParams, MapsetTagsEntries},
    Database,
};
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;
use twilight_model::id::{marker::UserMarker, Id};

use crate::core::Context;

#[derive(Copy, Clone)]
pub struct GameManager {
    psql: &'static Database,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            psql: Context::psql(),
        }
    }
}

impl GameManager {
    pub async fn higherlower_leaderboard(self, version: HlVersion) -> Result<Vec<HlGameScore>> {
        self.psql
            .select_higherlower_scores_by_version(version as i16)
            .await
            .wrap_err("failed to get higherlower leaderboard")
    }

    pub async fn higherlower_highscore(
        self,
        user_id: Id<UserMarker>,
        version: HlVersion,
    ) -> Result<u32> {
        self.psql
            .select_higherlower_highscore(user_id, version as i16)
            .await
            .wrap_err("failed to get higherlower highscore")
    }

    pub async fn upsert_higherlower_score(
        self,
        user_id: Id<UserMarker>,
        version: HlVersion,
        score: u32,
    ) -> Result<bool> {
        self.psql
            .upsert_higherlower_highscore(user_id, version as i16, score)
            .await
            .wrap_err("Failed to upsert higherlower score")
    }

    pub async fn bggame_leaderboard(self) -> Result<Vec<BgGameScore>> {
        self.psql
            .select_bggame_scores()
            .await
            .wrap_err("failed to get bggame leaderboard")
    }

    pub async fn bggame_tags(self, params: DbMapTagsParams) -> Result<MapsetTagsEntries> {
        let mode = params.mode;

        let tags = self
            .psql
            .select_map_tags(params)
            .await
            .wrap_err("Failed to get map tags")?;

        Ok(MapsetTagsEntries { mode, tags })
    }

    pub async fn bggame_increment_scores(
        self,
        scores: &HashMap<Id<UserMarker>, u32, IntHasher>,
    ) -> Result<()> {
        let mut user_ids = Vec::with_capacity(scores.len());
        let mut amounts = Vec::with_capacity(scores.len());

        for (user_id, amount) in scores {
            user_ids.push(user_id.get() as i64);
            amounts.push(*amount as i32);
        }

        self.psql
            .increment_bggame_scores(&user_ids, &amounts)
            .await
            .wrap_err("failed to increment score")
    }

    pub async fn bggame_upsert_mapset(
        self,
        mapset_id: u32,
        filename: &str,
        mode: GameMode,
    ) -> Result<()> {
        self.psql
            .upsert_map_tag(mapset_id, filename, mode)
            .await
            .wrap_err("failed to upsert mapset")
    }
}
