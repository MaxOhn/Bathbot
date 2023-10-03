use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
};

use bathbot_macros::SlashCommand;
use bathbot_model::{PullRequest, PullRequests, PullRequestsAndTags, ReferencedIssue};
use bathbot_util::constants::{FIELD_VALUE_SIZE, GENERAL_ISSUE};
use eyre::{ContextCompat, Result, WrapErr};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    active::{impls::ChangelogPagination, ActiveMessages},
    core::Context,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "changelog", desc = "Show all recent changes to the bot")]
pub struct Changelog;

async fn slash_changelog(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let PullRequestsAndTags {
        pull_requests:
            PullRequests {
                inner: mut pull_requests,
                mut next_cursor,
            },
        tags,
    } = match ctx.github().tags_and_prs().await {
        Ok(res) => res,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if tags.len() != 26 {
        let _ = command.error(&ctx, GENERAL_ISSUE).await;

        bail!("Expected 26 tags, got {}", tags.len());
    }

    let pages_fut = ChangelogTagPages::new(
        &ctx,
        &mut pull_requests,
        tags[0].date,
        tags[1].date,
        &mut next_cursor,
    );

    let pages = match pages_fut.await {
        Ok(res) => res,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to build pages"));
        }
    };

    let pagination = ChangelogPagination::new(
        tags,
        vec![pages],
        pull_requests,
        next_cursor,
        command.user_id()?,
    );

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}

#[derive(Debug, Default)]
pub struct ChangelogTagPage {
    pub features: Vec<Box<str>>,
    pub fixes: Vec<Box<str>>,
    pub adjustments: Vec<Box<str>>,
    pub other: Vec<Box<str>>,
}

#[derive(Debug)]
pub struct ChangelogTagPages {
    pub pages: Box<[ChangelogTagPage]>,
}

impl ChangelogTagPages {
    pub async fn new(
        ctx: &Context,
        pull_requests: &mut Vec<PullRequest>,
        start_date: OffsetDateTime,
        end_date: OffsetDateTime,
        next_cursor: &mut Box<str>,
    ) -> Result<Self> {
        let start_idx = pull_requests
            .iter()
            .position(|pr| pr.merged_at < start_date)
            .wrap_err("Found no PR before the start date")?;

        if start_idx > 0 {
            pull_requests.drain(0..start_idx);
        }

        let end_idx = loop {
            match pull_requests.iter().position(|pr| pr.merged_at < end_date) {
                Some(idx) => break idx,
                None => {
                    let mut next_prs = ctx
                        .github()
                        .next_prs(next_cursor)
                        .await
                        .wrap_err("Failed to get next pull requests")?
                        .into_original();

                    *next_cursor = next_prs.next_cursor;
                    pull_requests.append(&mut next_prs.inner);
                }
            }
        };

        let (prs, _) = pull_requests.split_at_mut(end_idx);
        prs.sort_unstable_by(|a, b| a.title.cmp(&b.title));

        let mut pages: Vec<ChangelogTagPage> = Vec::new();

        macro_rules! push_pr {
            ( $kind:ident, $pr:ident ) => {
                if let Some(page) = pages.iter_mut().find(|page| {
                    page.$kind.iter().fold(0, |len, s| len + s.len() + 2) + $pr.len()
                        <= FIELD_VALUE_SIZE
                }) {
                    page.$kind.push($pr);
                } else {
                    let mut page = ChangelogTagPage::default();
                    page.$kind.push($pr);
                    pages.push(page);
                }
            };
        }

        for pr in pull_requests.drain(..end_idx) {
            let Some((prefix, title)) = pr.title.split_once(": ") else {
                let pr = format!(
                    "- [`#{id}{pr_by}`]({pr_url}) {title}{issue_by}",
                    id = pr.id,
                    pr_by = PullRequestBy(&pr.author_name),
                    pr_url = pr.url(),
                    title = pr.title,
                    issue_by = IssueBy(&pr.referenced_issues),
                );
                let pr = pr.into_boxed_str();
                push_pr!(other, pr);

                continue;
            };

            let (kind, _) = prefix
                .trim_end_matches(')')
                .split_once('(')
                .map_or((prefix, None), |(kind, projects)| (kind, Some(projects)));

            let pr = format!(
                "- [`#{id}{pr_by}`]({pr_url}) {title}{issue_by}",
                id = pr.id,
                pr_by = PullRequestBy(&pr.author_name),
                pr_url = pr.url(),
                issue_by = IssueBy(&pr.referenced_issues),
            );
            let pr = pr.into_boxed_str();

            match kind {
                "feat" => push_pr!(features, pr),
                "fix" => push_pr!(fixes, pr),
                "refactor" | "dep" | "ci" | "perf" | "chore" | "doc" => push_pr!(adjustments, pr),
                _ => push_pr!(other, pr),
            }
        }

        Ok(Self {
            pages: pages.into_boxed_slice(),
        })
    }
}

struct PullRequestBy<'a>(&'a str);

impl Display for PullRequestBy<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.0 {
            "MaxOhn" => Ok(()),
            author => write!(f, " ({author})"),
        }
    }
}

struct IssueBy<'a>(&'a [ReferencedIssue]);

impl Display for IssueBy<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut iter = self.0.iter();

        let Some(issue) = iter.next() else {
            return Ok(());
        };

        write!(
            f,
            " ([`{reporter}`]({url})",
            reporter = IssueReporter::new(issue),
            url = issue.url()
        )?;

        for issue in iter {
            write!(
                f,
                ", [`{reporter}`]({url})",
                reporter = IssueReporter::new(issue),
                url = issue.url()
            )?;
        }

        f.write_str(")")
    }
}

enum IssueReporter<'a> {
    Name(&'a str),
    Id(u64),
}

impl<'a> IssueReporter<'a> {
    fn new(issue: &'a ReferencedIssue) -> Self {
        if !matches!(issue.author_name.as_ref(), "Bathbot-Helper" | "MaxOhn") {
            return Self::Name(issue.author_name.as_ref());
        }

        issue
            .body
            .rsplit_once("by @")
            .map(|(_, reporter)| reporter)
            .filter(|reporter| !reporter.ends_with("adewanne3"))
            .map_or_else(|| IssueReporter::Id(issue.id), IssueReporter::Name)
    }
}

impl Display for IssueReporter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            IssueReporter::Name(name) => write!(f, "@{name}"),
            IssueReporter::Id(id) => write!(f, "#{id}"),
        }
    }
}
