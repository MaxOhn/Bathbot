use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use rkyv::Archive;
use serde::{
    de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use time::OffsetDateTime;

use crate::{deser::Datetime, rkyv_util::time::DateTimeRkyv};

pub struct GraphQLResponse<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for GraphQLResponse<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct QueryVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for QueryVisitor<T> {
            type Value = GraphQLResponse<T>;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with a data field")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct GraphQLData<T>(T);

                impl<'de, T: Deserialize<'de>> Deserialize<'de> for GraphQLData<T> {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct DataVisitor<T>(PhantomData<T>);

                        impl<'de, T: Deserialize<'de>> Visitor<'de> for DataVisitor<T> {
                            type Value = GraphQLData<T>;

                            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                                f.write_str("an object with a repository field")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                let mut repository = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "repository" => {
                                            repository = Some(map.next_value()?);
                                        }
                                        _ => {
                                            return Err(DeError::invalid_value(
                                                Unexpected::Str(key),
                                                &"repository",
                                            ))
                                        }
                                    }
                                }

                                repository
                                    .ok_or_else(|| DeError::missing_field("repository"))
                                    .map(GraphQLData)
                            }
                        }

                        d.deserialize_map(DataVisitor(PhantomData))
                    }
                }

                let mut data = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "data" => {
                            let GraphQLData(inner) = map.next_value()?;
                            data = Some(inner);
                        }
                        _ => return Err(DeError::invalid_value(Unexpected::Str(key), &"data")),
                    }
                }

                data.ok_or_else(|| DeError::missing_field("data"))
                    .map(GraphQLResponse)
            }
        }

        d.deserialize_map(QueryVisitor(PhantomData))
    }
}

pub struct PullRequestsAndTags {
    pub pull_requests: PullRequests,
    pub tags: Vec<Tag>,
}

impl<'de> Deserialize<'de> for PullRequestsAndTags {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct GraphQLDataVisitor;

        impl<'de> Visitor<'de> for GraphQLDataVisitor {
            type Value = PullRequestsAndTags;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with tags and pullRequests fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct Tags(Vec<Tag>);

                impl<'de> Deserialize<'de> for Tags {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        let mut tags = Vec::with_capacity(25);
                        let visitor = OnlyNodesVisitor { seq: &mut tags };
                        d.deserialize_map(visitor)?;

                        Ok(Self(tags))
                    }
                }

                let mut tags = None;
                let mut pull_requests = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "tags" => {
                            let Tags(vec) = map.next_value()?;
                            tags = Some(vec);
                        }
                        "pullRequests" => pull_requests = Some(map.next_value()?),
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"tags or pullRequests",
                            ))
                        }
                    }
                }

                let tags = tags.ok_or_else(|| DeError::missing_field("tags"))?;
                let pull_requests =
                    pull_requests.ok_or_else(|| DeError::missing_field("pullRequests"))?;

                Ok(PullRequestsAndTags {
                    pull_requests,
                    tags,
                })
            }
        }

        d.deserialize_map(GraphQLDataVisitor)
    }
}

pub struct OnlyPullRequests(pub PullRequests);

impl<'de> Deserialize<'de> for OnlyPullRequests {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct GraphQLDataVisitor;

        impl<'de> Visitor<'de> for GraphQLDataVisitor {
            type Value = OnlyPullRequests;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with a pullRequests field")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut pull_requests = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "pullRequests" => pull_requests = Some(map.next_value()?),
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"pullRequests",
                            ))
                        }
                    }
                }

                pull_requests
                    .ok_or_else(|| DeError::missing_field("pullRequests"))
                    .map(OnlyPullRequests)
            }
        }

        d.deserialize_map(GraphQLDataVisitor)
    }
}

#[derive(Archive, rkyv::Deserialize, rkyv::Serialize)]
pub struct PullRequests {
    pub inner: Vec<PullRequest>,
    pub next_cursor: Box<str>,
}

impl<'de> Deserialize<'de> for PullRequests {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct NextCursor(Box<str>);

        struct PullRequestsVisitor<'s, T> {
            seq: &'s mut Vec<T>,
        }

        impl<'de, 's, T: Deserialize<'de>> Visitor<'de> for PullRequestsVisitor<'s, T> {
            type Value = NextCursor;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with a nodes and pageInfo fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct PageInfo(Box<str>);

                impl<'de> Deserialize<'de> for PageInfo {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct PageInfoVisitor;

                        impl<'de> Visitor<'de> for PageInfoVisitor {
                            type Value = PageInfo;

                            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                                f.write_str("an object with a nextCursor field")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                let mut next_cursor = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "nextCursor" => next_cursor = Some(map.next_value()?),
                                        _ => {
                                            return Err(DeError::invalid_value(
                                                Unexpected::Str(key),
                                                &"nextCursor",
                                            ))
                                        }
                                    }
                                }

                                next_cursor
                                    .ok_or_else(|| DeError::missing_field("nextCursor"))
                                    .map(PageInfo)
                            }
                        }

                        d.deserialize_map(PageInfoVisitor)
                    }
                }

                let mut got_nodes = false;
                let mut next_cursor = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "nodes" => {
                            map.next_value_seed(NodesSeqVisitor { seq: self.seq })?;
                            got_nodes = true;
                        }
                        "pageInfo" => {
                            let PageInfo(info) = map.next_value()?;
                            next_cursor = Some(info);
                        }
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"nodes or pageInfo",
                            ))
                        }
                    }
                }

                let next_cursor = next_cursor.ok_or_else(|| DeError::missing_field("pageInfo"))?;

                if got_nodes {
                    Ok(NextCursor(next_cursor))
                } else {
                    Err(DeError::missing_field("nodes"))
                }
            }
        }

        let mut inner = Vec::with_capacity(100);
        let visitor = PullRequestsVisitor { seq: &mut inner };
        let NextCursor(next_cursor) = d.deserialize_map(visitor)?;

        Ok(Self { inner, next_cursor })
    }
}
#[derive(Archive, rkyv::Deserialize, rkyv::Serialize)]
pub struct PullRequest {
    pub author_name: Box<str>,
    pub id: u64,
    #[with(DateTimeRkyv)]
    pub merged_at: OffsetDateTime,
    pub referenced_issues: Vec<ReferencedIssue>,
    pub title: Box<str>,
}

impl PullRequest {
    pub fn url(&self) -> GithubUrlDisplay {
        GithubUrlDisplay::PullRequest(self.id)
    }
}

impl<'de> Deserialize<'de> for PullRequest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PullRequestVisitor;

        impl<'de> Visitor<'de> for PullRequestVisitor {
            type Value = PullRequest;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a pull request object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct ReferencedIssues(Vec<ReferencedIssue>);

                impl<'de> Deserialize<'de> for ReferencedIssues {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        let mut issues = Vec::with_capacity(1);
                        let visitor = OnlyNodesVisitor { seq: &mut issues };
                        d.deserialize_map(visitor)?;

                        Ok(Self(issues))
                    }
                }

                let mut author = None;
                let mut id = None;
                let mut merged_at = None;
                let mut referenced_issues = None;
                let mut title = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "author" => {
                            let AuthorLogin(login) = map.next_value()?;
                            author = Some(login);
                        }
                        "id" => id = Some(map.next_value()?),
                        "mergedAt" => {
                            let Datetime(datetime) = map.next_value()?;
                            merged_at = Some(datetime);
                        }
                        "referencedIssues" => {
                            let ReferencedIssues(vec) = map.next_value()?;
                            referenced_issues = Some(vec);
                        }
                        "title" => title = Some(map.next_value()?),
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"author, id, mergedAt, referencedIssues, or title",
                            ))
                        }
                    }
                }

                let author_name = author.ok_or_else(|| DeError::missing_field("author"))?;
                let id = id.ok_or_else(|| DeError::missing_field("id"))?;
                let merged_at = merged_at.ok_or_else(|| DeError::missing_field("mergedAt"))?;
                let referenced_issues =
                    referenced_issues.ok_or_else(|| DeError::missing_field("referencedIssues"))?;
                let title = title.ok_or_else(|| DeError::missing_field("title"))?;

                Ok(PullRequest {
                    author_name,
                    id,
                    merged_at,
                    referenced_issues,
                    title,
                })
            }
        }

        d.deserialize_map(PullRequestVisitor)
    }
}

struct AuthorLogin(Box<str>);

impl<'de> Deserialize<'de> for AuthorLogin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct AuthorVisitor;

        impl<'de> Visitor<'de> for AuthorVisitor {
            type Value = AuthorLogin;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with a login field")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut login = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "login" => login = Some(map.next_value()?),
                        _ => return Err(DeError::invalid_value(Unexpected::Str(key), &"login")),
                    }
                }

                login
                    .ok_or_else(|| DeError::missing_field("login"))
                    .map(AuthorLogin)
            }
        }

        d.deserialize_map(AuthorVisitor)
    }
}

#[derive(Archive, rkyv::Deserialize, rkyv::Serialize)]
pub struct ReferencedIssue {
    pub author_name: Box<str>,
    pub body: Box<str>,
    pub id: u64,
}

impl ReferencedIssue {
    pub fn url(&self) -> GithubUrlDisplay {
        GithubUrlDisplay::Issue(self.id)
    }
}

impl<'de> Deserialize<'de> for ReferencedIssue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ReferencedIssueVisitor;

        impl<'de> Visitor<'de> for ReferencedIssueVisitor {
            type Value = ReferencedIssue;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("a referenced issue object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut author = None;
                let mut body = None;
                let mut id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "author" => {
                            let AuthorLogin(login) = map.next_value()?;
                            author = Some(login);
                        }
                        "body" => body = Some(map.next_value()?),
                        "id" => id = Some(map.next_value()?),
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"author, body, or id",
                            ))
                        }
                    }
                }

                let author_name = author.ok_or_else(|| DeError::missing_field("author"))?;
                let body = body.ok_or_else(|| DeError::missing_field("body"))?;
                let id = id.ok_or_else(|| DeError::missing_field("id"))?;

                Ok(ReferencedIssue {
                    author_name,
                    body,
                    id,
                })
            }
        }

        d.deserialize_map(ReferencedIssueVisitor)
    }
}

pub struct Tag {
    pub name: Box<str>,
    pub date: OffsetDateTime,
}

impl<'de> Deserialize<'de> for Tag {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct TagVisitor;

        impl<'de> Visitor<'de> for TagVisitor {
            type Value = Tag;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with name and commit fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                struct CommitDate(OffsetDateTime);

                impl<'de> Deserialize<'de> for CommitDate {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct CommitDateVisitor;

                        impl<'de> Visitor<'de> for CommitDateVisitor {
                            type Value = CommitDate;

                            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                                f.write_str("an object with a date field")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                let mut date = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "date" => {
                                            let Datetime(datetime) = map.next_value()?;
                                            date = Some(datetime);
                                        }
                                        _ => {
                                            return Err(DeError::invalid_value(
                                                Unexpected::Str(key),
                                                &"date",
                                            ))
                                        }
                                    }
                                }

                                date.ok_or_else(|| DeError::missing_field("date"))
                                    .map(CommitDate)
                            }
                        }

                        d.deserialize_map(CommitDateVisitor)
                    }
                }

                let mut name = None;
                let mut date = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "name" => name = Some(map.next_value()?),
                        "commit" => {
                            let CommitDate(datetime) = map.next_value()?;
                            date = Some(datetime);
                        }
                        _ => {
                            return Err(DeError::invalid_value(
                                Unexpected::Str(key),
                                &"name or commit",
                            ))
                        }
                    }
                }

                let name = name.ok_or_else(|| DeError::missing_field("name"))?;
                let date = date.ok_or_else(|| DeError::missing_field("commit"))?;

                Ok(Tag { name, date })
            }
        }

        d.deserialize_map(TagVisitor)
    }
}

struct OnlyNodesVisitor<'s, T> {
    seq: &'s mut Vec<T>,
}

impl<'de, 's, T: Deserialize<'de>> Visitor<'de> for OnlyNodesVisitor<'s, T> {
    type Value = ();

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("an object with a nodes field of type sequence")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut got_nodes = false;

        while let Some(key) = map.next_key()? {
            match key {
                "nodes" => {
                    map.next_value_seed(NodesSeqVisitor { seq: self.seq })?;
                    got_nodes = true;
                }
                _ => return Err(DeError::invalid_value(Unexpected::Str(key), &"nodes")),
            }
        }

        if got_nodes {
            Ok(())
        } else {
            Err(DeError::missing_field("nodes"))
        }
    }
}

struct NodesSeqVisitor<'s, T> {
    seq: &'s mut Vec<T>,
}

impl<'de, 's, T: Deserialize<'de>> DeserializeSeed<'de> for NodesSeqVisitor<'s, T> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de, 's, T: Deserialize<'de>> Visitor<'de> for NodesSeqVisitor<'s, T> {
    type Value = ();

    fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("a sequence of objects")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while let Some(value) = seq.next_element()? {
            self.seq.push(value);
        }

        Ok(())
    }
}

pub enum GithubUrlDisplay {
    Issue(u64),
    PullRequest(u64),
}

impl Display for GithubUrlDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("https://github.com/MaxOhn/Bathbot/")?;

        match self {
            GithubUrlDisplay::Issue(id) => {
                write!(f, "issues/{id}")
            }
            GithubUrlDisplay::PullRequest(id) => {
                write!(f, "pull/{id}")
            }
        }
    }
}
