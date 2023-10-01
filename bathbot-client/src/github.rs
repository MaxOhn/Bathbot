use bathbot_model::{GraphQLQuery, PullRequests, PullRequestsAndTags};
use eyre::{Result, WrapErr};

use crate::{site::Site, Client};

impl Client {
    pub async fn github_pull_requests_and_tags(&self) -> Result<PullRequestsAndTags> {
        let url = "TODO";
        let json = Vec::new();

        let bytes = self.make_json_post_request(url, Site::Github, json).await?;

        let GraphQLQuery(data) = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize github pull requests and tags: {body}")
        })?;

        Ok(data)
    }

    pub async fn github_pull_requests(&self, next_cursor: &str) -> Result<PullRequests> {
        let url = "TODO";
        let json = Vec::new();

        let bytes = self.make_json_post_request(url, Site::Github, json).await?;

        let GraphQLQuery(data) = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize github pull requests: {body}")
        })?;

        Ok(data)
    }
}

/*
query ($cursor: String) {
  repository(owner: "MaxOhn", name: "Bathbot") {
    tags: refs(
      refPrefix: "refs/tags/"
      first: 25
      orderBy: {field: TAG_COMMIT_DATE, direction: DESC}
    ) {
      nodes {
        name
        commit: target {
          ... on Commit {
            date: committedDate
          }
        }
      }
    }
    pullRequests(
      after: $cursor
      first: 100
      states: MERGED
      orderBy: {field: UPDATED_AT, direction: DESC}
    ) {
      nodes {
        author {
          login
        }
        referencedIssues: closingIssuesReferences(first: 10) {
          nodes {
            body: bodyText
            id: number
            url
          }
        }
        mergedAt
        id: number
        title
        url
      }
      pageInfo {
        nextCursor: endCursor
      }
    }
  }
}

-------------------------

query ($cursor: String!) {
  repository(owner: "MaxOhn", name: "Bathbot") {
    pullRequests(
      after: $cursor
      first: 100
      states: MERGED
      orderBy: {field: UPDATED_AT, direction: DESC}
    ) {
      nodes {
        author {
          login
        }
        referencedIssues: closingIssuesReferences(first: 5) {
          nodes {
            body: bodyText
            id: number
            url
          }
        }
        mergedAt
        id: number
        title
        url
      }
      pageInfo {
        nextCursor: endCursor
      }
    }
  }
}
*/
