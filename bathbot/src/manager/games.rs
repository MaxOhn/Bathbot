use bathbot_model::{HlVersion, MapsetTagsEntries};
use bathbot_psql::{
    model::games::{DbBgGameScore, DbHlGameScore, DbMapTagsParams},
    Database,
};
use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;
use twilight_model::id::{marker::UserMarker, Id};

#[derive(Copy, Clone)]
pub struct GameManager<'d> {
    psql: &'d Database,
}

impl<'d> GameManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
    }
}

impl GameManager<'_> {
    pub async fn higherlower_leaderboard(self, version: HlVersion) -> Result<Vec<DbHlGameScore>> {
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
            .wrap_err("failed to upsert higherlower score")
    }
}

impl GameManager<'_> {
    pub async fn bggame_leaderboard(self) -> Result<Vec<DbBgGameScore>> {
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
            .wrap_err("failed to get map tags")?;

        Ok(MapsetTagsEntries { mode, tags })
    }

    pub async fn bggame_increment_score(self, user_id: Id<UserMarker>, amount: u32) -> Result<()> {
        self.psql
            .increment_bggame_score(user_id, amount as i32)
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
