use bathbot_psql::Database;
use eyre::{Result, WrapErr};
use futures::future;
use rosu_v2::{
    prelude::{OsuError, Score},
    OsuResult,
};

use crate::core::Context;

use super::redis::{
    osu::{User, UserArgs, UserArgsSlim},
    RedisData,
};

#[derive(Copy, Clone)]
pub struct ScoresManager<'c> {
    ctx: &'c Context,
    psql: &'c Database,
}

impl<'c> ScoresManager<'c> {
    pub fn new(ctx: &'c Context, psql: &'c Database) -> Self {
        Self { ctx, psql }
    }

    pub fn top(self) -> ScoreArgs<'c> {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Top { limit: 100 },
        }
    }

    pub fn recent(self) -> ScoreArgs<'c> {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Recent {
                limit: 100,
                include_fails: true,
            },
        }
    }

    pub fn pinned(self) -> ScoreArgs<'c> {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::Pinned { limit: 100 },
        }
    }

    pub fn user_on_map(self, map_id: u32) -> ScoreArgs<'c> {
        ScoreArgs {
            manager: self,
            kind: ScoreKind::UserMap { map_id },
        }
    }

    async fn store(self, scores: &[Score]) -> Result<()> {
        self.psql
            .insert_scores(scores)
            .await
            .wrap_err("failed to store top scores")
    }
}

#[derive(Copy, Clone)]
pub struct ScoreArgs<'c> {
    manager: ScoresManager<'c>,
    kind: ScoreKind,
}

#[derive(Copy, Clone)]
enum ScoreKind {
    Top { limit: usize },
    Recent { limit: usize, include_fails: bool },
    Pinned { limit: usize },
    UserMap { map_id: u32 },
}

impl<'c> ScoreArgs<'c> {
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
        let ctx = self.manager.ctx;

        // Retrieve score(s)
        let scores_res = match self.kind {
            ScoreKind::Top { limit } => {
                ctx.osu()
                    .user_scores(user_id)
                    .best()
                    .limit(limit)
                    .mode(mode)
                    .await
            }
            ScoreKind::Recent {
                limit,
                include_fails,
            } => {
                ctx.osu()
                    .user_scores(user_id)
                    .recent()
                    .limit(limit)
                    .mode(mode)
                    .include_fails(include_fails)
                    .await
            }
            ScoreKind::Pinned { limit } => {
                ctx.osu()
                    .user_scores(user_id)
                    .pinned()
                    .limit(limit)
                    .mode(mode)
                    .await
            }
            ScoreKind::UserMap { map_id } => {
                ctx.osu()
                    .beatmap_user_scores(map_id, user_id)
                    .mode(mode)
                    .await
            }
        };

        // Execute score retrieval
        let scores = match scores_res {
            Ok(scores) => scores,
            Err(OsuError::NotFound) => {
                // Remove stats of unknown/restricted users so they don't appear in the leaderboard
                if let Err(err) = ctx.osu_user().remove_stats(user_id).await {
                    let wrap = "Failed to remove stats of unknown user";
                    warn!("{:?}", err.wrap_err(wrap));
                }

                return Err(OsuError::NotFound);
            }
            Err(err) => return Err(err),
        };

        // Store scores in database
        let store_fut = async {
            match self.kind {
                ScoreKind::Top { .. } | ScoreKind::UserMap { .. } | ScoreKind::Pinned { .. } => {
                    self.manager.store(&scores).await
                }
                ScoreKind::Recent { .. } => future::ready(Ok(())).await,
            }
        };

        // Pass scores to tracking check
        let tracking_fut = async {
            #[cfg(feature = "osutracking")]
            if let ScoreKind::Top { .. } = self.kind {
                crate::tracking::process_osu_tracking(ctx, &scores, None).await
            }
        };

        let (store_res, _) = tokio::join!(store_fut, tracking_fut);

        if let Err(err) = store_res {
            warn!("{:?}", err.wrap_err("Failed to store top scores"));
        }

        Ok(scores)
    }

    pub async fn exec_with_user(
        self,
        user_args: UserArgs,
    ) -> OsuResult<(RedisData<User>, Vec<Score>)> {
        match user_args {
            UserArgs::Args(args) => {
                let user_fut = self.manager.ctx.redis().osu_user_from_args(args);
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
