use bathbot_cache::Cache;
use bathbot_model::{PullRequests, PullRequestsAndTags};
use eyre::{Result, WrapErr};

use crate::{core::Context, manager::redis::RedisData};

pub struct GithubManager<'a> {
    ctx: &'a Context,
}

impl<'a> GithubManager<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self { ctx }
    }
}

impl GithubManager<'_> {
    pub async fn tags_and_prs(&self) -> Result<PullRequestsAndTags> {
        self.ctx
            .client()
            .github_pull_requests_and_tags()
            .await
            .wrap_err("Failed to get tags and PRs")
    }

    pub async fn next_prs(&self, next_cursor: &str) -> Result<RedisData<PullRequests>> {
        const EXPIRE: usize = 1800; // 30 min
        let key = format!("github_prs_{next_cursor}");

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(prs)) => {
                self.ctx.stats.inc_cached_github_prs();

                return Ok(RedisData::Archive(prs));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let prs = self.ctx.client().github_pull_requests(next_cursor).await?;

        if let Some(ref mut conn) = conn {
            // TODO: check scratch size
            if let Err(err) = Cache::store::<_, _, 1024>(conn, &key, &prs, EXPIRE).await {
                warn!(?err, "Failed to store github pull requests");
            }
        }

        Ok(RedisData::new(prs))
    }
}
