use std::slice;

use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::{DbScores, DbScoresBuilder, DbTopScores};
use bathbot_util::{osu::ModSelection, IntHasher};
use eyre::{Result, WrapErr};
use rosu_v2::{
    model::score::BeatmapUserScore,
    prelude::{GameMode, GameModsIntermode, Grade, OsuError, Score},
    OsuResult,
};

use super::redis::{
    osu::{UserArgs, UserArgsSlim},
    RedisData,
};
use crate::core::Context;

#[derive(Clone)]
pub struct ScoresManager;

impl ScoresManager {
    pub fn new() -> Self {
        Self
    }

    fn scores_builder<'a>(
        mode: Option<GameMode>,
        mods: Option<&ModSelection>,
        country_code: Option<&'a str>,
        map_id: Option<u32>,
        grade: Option<Grade>,
    ) -> DbScoresBuilder<'a> {
        let mut builder = DbScoresBuilder::new();

        if let Some(mode) = mode {
            builder.mode(mode);
        }

        if let Some(country_code) = country_code {
            builder.country_code(country_code);
        }

        if let Some(map_id) = map_id {
            builder.map_id(map_id as i32);
        }

        if let Some(grade) = grade {
            builder.grade(grade);
        }

        if let Some(mods) = mods {
            match mods {
                ModSelection::Include(mods) => builder.mods_include(mods.bits() as i32),
                ModSelection::Exclude(mods) => builder.mods_exclude(mods.bits() as i32),
                ModSelection::Exact(mods) => builder.mods_exact(mods.bits() as i32),
            };
        }

        builder
    }

    #[allow(clippy::wrong_self_convention)]
    pub async fn from_discord_ids(
        self,
        users: &[i64],
        mode: Option<GameMode>,
        mods: Option<&ModSelection>,
        country_code: Option<&str>,
        map_id: Option<u32>,
        grade: Option<Grade>,
    ) -> Result<DbScores<IntHasher>> {
        Self::scores_builder(mode, mods, country_code, map_id, grade)
            .build_discord(Context::psql(), users)
            .await
            .wrap_err("Failed to select scores")
    }

    #[allow(clippy::wrong_self_convention)]
    pub async fn from_osu_ids(
        self,
        users: &[i32],
        mode: Option<GameMode>,
        mods: Option<&ModSelection>,
        country_code: Option<&str>,
        map_id: Option<u32>,
        grade: Option<Grade>,
    ) -> Result<DbScores<IntHasher>> {
        Self::scores_builder(mode, mods, country_code, map_id, grade)
            .build_osu(Context::psql(), users)
            .await
            .wrap_err("Failed to select scores")
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

    pub async fn db_top_scores(
        self,
        mode: GameMode,
        user_ids: Option<&[i32]>,
        country_code: Option<&str>,
    ) -> Result<DbTopScores<IntHasher>> {
        Context::psql()
            .select_top100_scores(mode, country_code, user_ids)
            .await
            .wrap_err("Failed to fetch top scores")
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
        if let Err(err) = Context::psql().insert_scores(scores).await {
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

        tokio::spawn(async move {
            // Store scores in database
            self.manager.store(&scores_clone).await;

            // Pass scores to tracking check
            #[cfg(feature = "osutracking")]
            if let ScoreKind::Top { .. } = self.kind {
                crate::tracking::process_osu_tracking(&scores_clone, None).await
            }
        });

        Ok(scores)
    }

    pub async fn exec_with_user(
        self,
        user_args: UserArgs,
    ) -> OsuResult<(RedisData<User>, Vec<Score>)> {
        match user_args {
            UserArgs::Args(args) => {
                let user_fut = Context::redis().osu_user_from_args(args);
                let score_fut = self.exec(args);

                tokio::try_join!(user_fut, score_fut)
            }
            UserArgs::User { user, mode } => {
                let args = UserArgsSlim::user_id(user.user_id).mode(mode);
                let user = RedisData::Original(*user);
                let scores = self.exec(args).await?;

                Ok((user, scores))
            }
            UserArgs::Err(err) => Err(err),
        }
    }
}
