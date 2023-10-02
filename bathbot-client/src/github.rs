use bathbot_model::{GraphQLResponse, OnlyPullRequests, PullRequests, PullRequestsAndTags};
use eyre::{Result, WrapErr};
use serde::Serialize;

use crate::{site::Site, Client};

const URL: &str = "https://api.github.com/graphql";

impl Client {
    pub async fn github_pull_requests_and_tags(&self) -> Result<PullRequestsAndTags> {
        let query = r#"
query {
  repository(owner: "MaxOhn", name: "Bathbot") {
    tags: refs(
      refPrefix: "refs/tags/"
      first: 26
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
      first: 100
      states: MERGED
      orderBy: {field: CREATED_AT, direction: DESC}
    ) {
      nodes {
        author {
          login
        }
        id: number
        mergedAt
        referencedIssues: closingIssuesReferences(first: 10) {
          nodes {
            author {
              login
            }
            body: bodyText
            id: number
          }
        }
        title
      }
      pageInfo {
        nextCursor: endCursor
      }
    }
  }
}"#;

        let query = GraphQLQuery::new(query);
        let json = serde_json::to_vec(&query).unwrap();
        let bytes = self.make_json_post_request(URL, Site::Github, json).await?;

        let GraphQLResponse(data) = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize github pull requests and tags: {body}")
        })?;

        Ok(data)
    }

    pub async fn github_pull_requests(&self, next_cursor: &str) -> Result<PullRequests> {
        let query = r#"
query ($cursor: String!) {
  repository(owner: "MaxOhn", name: "Bathbot") {
    pullRequests(
      after: $cursor
      first: 100
      states: MERGED
      orderBy: {field: CREATED_AT, direction: DESC}
    ) {
      nodes {
        author {
          login
        }
        id: number
        mergedAt
        referencedIssues: closingIssuesReferences(first: 10) {
          nodes {
            author {
              login
            }
            body: bodyText
            id: number
          }
        }
        title
      }
      pageInfo {
        nextCursor: endCursor
      }
    }
  }
}"#;

        #[derive(Serialize)]
        struct Variables<'s> {
            cursor: &'s str,
        }

        let variables = Variables {
            cursor: next_cursor,
        };
        let query = GraphQLQuery::new(query).with_variables(variables);
        let json = serde_json::to_vec(&query).unwrap();
        let bytes = self.make_json_post_request(URL, Site::Github, json).await?;

        let GraphQLResponse(OnlyPullRequests(data)) = serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize github pull requests: {body}")
            })?;

        Ok(data)
    }
}

#[derive(Serialize)]
struct GraphQLQuery<'s, V> {
    query: &'s str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<V>,
}

impl<'s> GraphQLQuery<'s, ()> {
    fn new(query: &'s str) -> Self {
        Self {
            query,
            variables: None,
        }
    }

    fn with_variables<V>(self, variables: V) -> GraphQLQuery<'s, V> {
        GraphQLQuery {
            query: self.query,
            variables: Some(variables),
        }
    }
}
