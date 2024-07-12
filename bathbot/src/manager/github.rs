use bathbot_cache::Cache;
use bathbot_model::{PullRequests, PullRequestsAndTags};
use eyre::{Result, WrapErr};

use crate::{
    core::{BotMetrics, Context},
    manager::redis::RedisData,
};

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

    pub async fn next_prs(self, next_cursor: &str) -> Result<RedisData<PullRequests>> {
        const EXPIRE: u64 = 1800; // 30 min
        let key = format!("github_prs_{next_cursor}");

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(prs)) => {
                BotMetrics::inc_redis_hit("github prs");

                return Ok(RedisData::Archive(prs));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let prs = Context::client().github_pull_requests(next_cursor).await?;

        if let Some(ref mut conn) = conn {
            // TODO: check scratch size
            if let Err(err) = Cache::store::<_, _, 1024>(conn, &key, &prs, EXPIRE).await {
                warn!(?err, "Failed to store github pull requests");
            }
        }

        Ok(RedisData::new(prs))
    }
}
