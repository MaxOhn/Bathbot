use bathbot_cache::{Cache, util::serialize::serialize_using_arena};
use bathbot_model::{ArchivedPullRequests, PullRequests, PullRequestsAndTags};
use eyre::{Report, Result, WrapErr};

use super::redis::RedisError;
use crate::core::{BotMetrics, Context};

#[derive(Copy, Clone)]
pub struct GithubManager;

impl GithubManager {
    pub fn new() -> Self {
        Self
    }
}

impl GithubManager {
    pub async fn tags_and_prs(self) -> Result<PullRequestsAndTags> {
        Context::client()
            .github_pull_requests_and_tags()
            .await
            .wrap_err("Failed to get tags and PRs")
    }

    pub async fn next_prs(self, next_cursor: &str) -> Result<PullRequests> {
        const EXPIRE: u64 = 1800; // 30 min
        let key = format!("github_prs_{next_cursor}");

        let mut conn = match Context::cache()
            .fetch::<_, ArchivedPullRequests>(&key)
            .await
        {
            Ok(Ok(prs)) => {
                BotMetrics::inc_redis_hit("github prs");

                return prs.try_deserialize().map_err(Report::new);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch github prs");

                None
            }
        };

        let prs = Context::client().github_pull_requests(next_cursor).await?;

        if let Some(ref mut conn) = conn {
            let bytes = serialize_using_arena(&prs).map_err(RedisError::Serialization)?;

            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store github prs");
            }
        }

        Ok(prs)
    }
}
