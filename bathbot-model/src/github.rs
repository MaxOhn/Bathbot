use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use serde::{
    de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use time::OffsetDateTime;

use crate::deser::datetime_z;

pub struct GraphQLQuery<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for GraphQLQuery<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct QueryVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for QueryVisitor<T> {
            type Value = GraphQLQuery<T>;

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
                    .map(GraphQLQuery)
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

pub struct PullRequests {
    pub inner: Vec<PullRequest>,
    pub next_cursor: Box<str>,
}

impl<'de> Deserialize<'de> for PullRequests {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct NextCursor(Box<str>);

        struct NodesVisitor<'s, T> {
            seq: &'s mut Vec<T>,
        }

        impl<'de, 's, T: Deserialize<'de>> Visitor<'de> for NodesVisitor<'s, T> {
            type Value = NextCursor;

            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.write_str("an object with a nodes and nextCursor fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut got_nodes = false;
                let mut next_cursor = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "nodes" => {
                            map.next_value_seed(NodesSeqVisitor { seq: self.seq })?;
                            got_nodes = true;
                        }
                        "nextCursor" => next_cursor = Some(map.next_value()?),
                        _ => return Err(DeError::invalid_value(Unexpected::Str(key), &"nodes")),
                    }
                }

                let next_cursor =
                    next_cursor.ok_or_else(|| DeError::missing_field("nextCursor"))?;

                if got_nodes {
                    Ok(NextCursor(next_cursor))
                } else {
                    Err(DeError::missing_field("nodes"))
                }
            }
        }

        let mut inner = Vec::with_capacity(100);
        let visitor = NodesVisitor { seq: &mut inner };
        let NextCursor(next_cursor) = d.deserialize_map(visitor)?;

        Ok(Self { inner, next_cursor })
    }
}

pub struct PullRequest {
    pub author_name: Box<str>,
    pub id: u64,
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
                struct Author(Box<str>);

                impl<'de> Deserialize<'de> for Author {
                    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct AuthorVisitor;

                        impl<'de> Visitor<'de> for AuthorVisitor {
                            type Value = Author;

                            fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                                f.write_str("an object with a login field")
                            }

                            fn visit_map<A: MapAccess<'de>>(
                                self,
                                mut map: A,
                            ) -> Result<Self::Value, A::Error> {
                                let mut login = None;

                                while let Some(key) = map.next_key()? {
                                    match key {
                                        "login" => login = Some(map.next_value()?),
                                        _ => {
                                            return Err(DeError::invalid_value(
                                                Unexpected::Str(key),
                                                &"login",
                                            ))
                                        }
                                    }
                                }

                                login
                                    .ok_or_else(|| DeError::missing_field("login"))
                                    .map(Author)
                            }
                        }

                        d.deserialize_map(AuthorVisitor)
                    }
                }

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
                            let Author(login) = map.next_value()?;
                            author = Some(login);
                        }
                        "id" => id = Some(map.next_value()?),
                        "referencedIssues" => {
                            let ReferencedIssues(vec) = map.next_value()?;
                            referenced_issues = Some(vec);
                        }
                        "mergedAt" => {
                            let Datetime(datetime) = map.next_value()?;
                            merged_at = Some(datetime);
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

                let author = author.ok_or_else(|| DeError::missing_field("author"))?;
                let id = id.ok_or_else(|| DeError::missing_field("id"))?;
                let merged_at = merged_at.ok_or_else(|| DeError::missing_field("mergedAt"))?;
                let referenced_issues =
                    referenced_issues.ok_or_else(|| DeError::missing_field("referencedIssues"))?;
                let title = title.ok_or_else(|| DeError::missing_field("title"))?;

                Ok(PullRequest {
                    author_name: author,
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

struct Datetime(OffsetDateTime);

impl<'de> Deserialize<'de> for Datetime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        datetime_z::deserialize(d).map(Self)
    }
}

pub struct ReferencedIssue {
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
                let mut body = None;
                let mut id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "body" => body = Some(map.next_value()?),
                        "id" => id = Some(map.next_value()?),
                        _ => {
                            return Err(DeError::invalid_value(Unexpected::Str(key), &"body or id"))
                        }
                    }
                }

                let body = body.ok_or_else(|| DeError::missing_field("body"))?;
                let id = id.ok_or_else(|| DeError::missing_field("id"))?;

                Ok(ReferencedIssue { body, id })
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
                f.write_str("an object with name and target fields")
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
                let date = date.ok_or_else(|| DeError::missing_field("target"))?;

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
