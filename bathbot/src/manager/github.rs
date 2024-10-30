use bathbot_cache::bathbot::github_pull_requests::CacheGithubPullRequests;
use bathbot_model::{PullRequests, PullRequestsAndTags};
use eyre::{Result, WrapErr};

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
        let key = format!("github_prs_{next_cursor}");

        match Context::cache()
            .fetch::<CacheGithubPullRequests>(&key)
            .await
        {
            Ok(Some(data)) => {
                BotMetrics::inc_redis_hit("github prs");

                return data.deserialize();
            }
            Ok(None) => {}
            Err(err) => warn!("{err:?}"),
        }

        let data = Context::client().github_pull_requests(next_cursor).await?;
        let store_fut = Context::cache().store::<CacheGithubPullRequests>(&key, &data);

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store {key}");
        }

        Ok(data)
    }
}
