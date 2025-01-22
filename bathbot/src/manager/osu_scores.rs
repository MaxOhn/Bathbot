use std::slice;

use eyre::{Result, WrapErr};
use rosu_v2::{
    model::score::BeatmapUserScore,
    prelude::{GameMode, GameModsIntermode, OsuError, Score},
    OsuResult,
};

use super::redis::osu::{CachedUser, UserArgs, UserArgsError, UserArgsSlim};
use crate::core::Context;

#[derive(Clone)]
pub struct ScoresManager;

impl ScoresManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn map_leaderboard(
        self,
        map_id: u32,
        mode: GameMode,
        mods: Option<GameModsIntermode>,
        limit: u32,
        legacy_scores: bool,
    ) -> Result<Vec<Score>> {
        let mut req = Context::osu()
            .beatmap_scores(map_id)
            .limit(limit)
            .mode(mode)
            .legacy_only(legacy_scores)
            .legacy_scores(legacy_scores);

        if let Some(mods) = mods {
            req = req.mods(mods);
        }

        let scores = req.await.wrap_err("Failed to get map leaderboard")?;

        let scores_clone = Box::from(scores.as_slice());
        tokio::spawn(async move { self.store(&scores_clone).await });

        Ok(scores)
    }

    pub async fn user_on_map_single(
        self,
        user_id: u32,
        map_id: u32,
        mode: GameMode,
        mods: Option<GameModsIntermode>,
        legacy_scores: bool,
    ) -> Result<BeatmapUserScore, OsuError> {
        let mut req = Context::osu()
            .beatmap_user_score(map_id, user_id)
            .mode(mode)
            .legacy_only(legacy_scores)
            .legacy_scores(legacy_scores);

        if let Some(mods) = mods {
            req = req.mods(mods);
        }

        let score = req.await?;

        let score_inner = score.score.clone();
        tokio::spawn(async move { self.store(slice::from_ref(&score_inner)).await });

        Ok(score)
    }

    pub fn top(self, legacy_scores: bool) -> ScoreArgs {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Top { limit: 100 },
            legacy_scores,
        }
    }

    pub fn recent(self, legacy_scores: bool) -> ScoreArgs {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Recent {
                limit: 100,
                include_fails: true,
            },
            legacy_scores,
        }
    }

    pub fn pinned(self, legacy_scores: bool) -> ScoreArgs {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Pinned { limit: 100 },
            legacy_scores,
        }
    }

    pub fn user_on_map(self, map_id: u32, legacy_scores: bool) -> ScoreArgs {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::UserMap { map_id },
            legacy_scores,
        }
    }

    async fn store(self, scores: &[Score]) {
        if let Err(err) = Context::psql().insert_scores_mapsets(scores).await {
            warn!(?err, "Failed to store scores");
        }
    }
}

#[derive(Clone)]
pub struct ScoreArgs {
    manager: ScoresManager,
    kind: ScoreKind,
    legacy_scores: bool,
}

#[derive(Copy, Clone)]
enum ScoreKind {
    Top { limit: usize },
    Recent { limit: usize, include_fails: bool },
    Pinned { limit: usize },
    UserMap { map_id: u32 },
}

impl ScoreArgs {
    pub fn limit(mut self, limit: usize) -> Self {
        match &mut self.kind {
            ScoreKind::Top { limit: limit_ } => *limit_ = limit,
            ScoreKind::Recent { limit: limit_, .. } => *limit_ = limit,
            ScoreKind::Pinned { limit: limit_, .. } => *limit_ = limit,
            ScoreKind::UserMap { .. } => {}
        }

        self
    }

    pub fn include_fails(mut self, include_fails: bool) -> Self {
        if let ScoreKind::Recent {
            include_fails: include_fails_,
            ..
        } = &mut self.kind
        {
            *include_fails_ = include_fails;
        }

        self
    }

    pub async fn exec(self, user_args: UserArgsSlim) -> OsuResult<Vec<Score>> {
        let UserArgsSlim { user_id, mode } = user_args;

        // Retrieve score(s)
        let scores_res = match self.kind {
            ScoreKind::Top { limit } => {
                Context::osu()
                    .user_scores(user_id)
                    .best()
                    .limit(limit)
                    .mode(mode)
                    .legacy_only(self.legacy_scores)
                    .legacy_scores(self.legacy_scores)
                    .await
            }
            ScoreKind::Recent {
                limit,
                include_fails,
            } => {
                Context::osu()
                    .user_scores(user_id)
                    .recent()
                    .limit(limit)
                    .mode(mode)
                    .include_fails(include_fails)
                    .legacy_only(self.legacy_scores)
                    .legacy_scores(self.legacy_scores)
                    .await
            }
            ScoreKind::Pinned { limit } => {
                Context::osu()
                    .user_scores(user_id)
                    .pinned()
                    .limit(limit)
                    .mode(mode)
                    .legacy_only(self.legacy_scores)
                    .legacy_scores(self.legacy_scores)
                    .await
            }
            ScoreKind::UserMap { map_id } => {
                Context::osu()
                    .beatmap_user_scores(map_id, user_id)
                    .mode(mode)
                    .legacy_only(self.legacy_scores)
                    .legacy_scores(self.legacy_scores)
                    .await
            }
        };

        // Execute score retrieval
        let scores = match scores_res {
            Ok(scores) => scores,
            Err(OsuError::NotFound) => {
                // Remove stats of unknown/restricted users so they don't appear in the
                // leaderboard
                if let Err(err) = Context::osu_user().remove_stats_and_scores(user_id).await {
                    warn!(?err, "Failed to remove stats of unknown user");
                }

                return Err(OsuError::NotFound);
            }
            Err(err) => return Err(err),
        };

        let scores_clone = Box::from(scores.as_slice());
        tokio::spawn(async move { self.manager.store(&scores_clone).await });

        Ok(scores)
    }

    pub async fn exec_with_user(
        self,
        user_args: UserArgs,
    ) -> Result<(CachedUser, Vec<Score>), UserArgsError> {
        match user_args {
            UserArgs::Args(args) => {
                let user_fut = Context::redis().osu_user_from_args(args);
                let score_fut = self.exec(args);

                let (user_res, score_res) = tokio::join!(user_fut, score_fut);

                Ok((user_res?, score_res?))
            }
            UserArgs::User { user, mode } => {
                let args = UserArgsSlim::user_id(user.user_id.to_native()).mode(mode);
                let scores = self.exec(args).await?;

                Ok((user, scores))
            }
            UserArgs::Err(err) => Err(err),
        }
    }
}
