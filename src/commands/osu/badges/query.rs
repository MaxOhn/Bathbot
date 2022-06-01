use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    fmt::Write,
    sync::Arc,
};

use eyre::Report;
use rkyv::{Deserialize, Infallible};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{ApplicationCommand, ApplicationCommandAutocomplete},
};

use crate::{
    core::Context,
    custom_client::OsekaiBadge,
    error::Error,
    pagination::BadgePagination,
    util::{
        constants::OSEKAI_ISSUE, get_combined_thumbnail, levenshtein_similarity,
        ApplicationCommandExt, AutocompleteExt, CowUtils,
    },
    BotResult,
};

use super::BadgesQuery;

pub(super) async fn query(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    args: BadgesQuery,
) -> BotResult<()> {
    let BadgesQuery { name, sort } = args;

    let badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let name_ = name.cow_to_ascii_lowercase();
    let name = name_.as_ref();
    let mut found_exact = false;

    let mut badges: Vec<OsekaiBadge> = badges
        .get()
        .iter()
        .scan(&mut found_exact, |found_exact, badge| {
            if **found_exact {
                None
            } else {
                let lowercase_name = badge.name.cow_to_ascii_lowercase();
                let lowercase_desc = badge.description.to_ascii_lowercase();

                if lowercase_name == name || lowercase_desc == name {
                    **found_exact = true;

                    Some(Some(badge))
                } else if lowercase_name.contains(name) || lowercase_desc.contains(name) {
                    Some(Some(badge))
                } else {
                    Some(None)
                }
            }
        })
        .filter_map(|badge| badge?.deserialize(&mut Infallible).ok())
        .collect();

    if found_exact && badges.len() > 1 {
        let len = badges.len();
        badges.swap(0, len - 1);
        badges.truncate(1);
    }

    sort.unwrap_or_default().apply(&mut badges);

    let owners = if let Some(badge) = badges.first() {
        let owners_fut = ctx.client().get_osekai_badge_owners(badge.badge_id);

        match owners_fut.await {
            Ok(owners) => owners,
            Err(err) => {
                let _ = command.error(&ctx, OSEKAI_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        return no_badge_found(&ctx, &command, name).await;
    };

    let urls = owners.iter().map(|owner| owner.avatar_url.as_str());

    let bytes = if badges.len() == 1 {
        match get_combined_thumbnail(&ctx, urls, owners.len() as u32, Some(1024)).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to combine avatars");
                warn!("{report:?}");

                None
            }
        }
    } else {
        None
    };

    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners);

    let mut builder = BadgePagination::builder(Arc::clone(&ctx), badges, owners_map);

    if let Some(bytes) = bytes {
        builder = builder.attachment("badge_owners.png", bytes);
    }

    builder.start_by_update().defer_components().start(ctx, command.into()).await
}

async fn no_badge_found(ctx: &Context, command: &ApplicationCommand, name: &str) -> BotResult<()> {
    let badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let archived_badges = badges.get();
    let mut heap = BinaryHeap::with_capacity(2 * archived_badges.len());

    for badge in archived_badges.iter() {
        heap.push(MatchingString::new_with_cow(name, &badge.name));
        heap.push(MatchingString::new(name, &badge.description));
    }

    let mut content = format!("No badge found that matches `{name}`.");

    if let Some(matching) = heap.pop().filter(|m| m.similarity > 0.0) {
        let _ = write!(content, " Did you mean `{}`", matching.value);
        let mut i = 1;

        while let Some(matching) = heap.pop().filter(|m| m.similarity > 0.0) {
            let _ = write!(content, ", `{}`", matching.value);
            i += 1;

            if i == 5 {
                break;
            }
        }

        content.push('?');
    }

    command.error(ctx, content).await?;

    Ok(())
}

pub async fn handle_autocomplete(
    ctx: Arc<Context>,
    command: Box<ApplicationCommandAutocomplete>,
) -> BotResult<()> {
    let value_opt = command
        .data
        .options
        .first()
        .and_then(|opt| opt.options.first())
        .and_then(|opt| opt.value.as_ref());

    let name = match value_opt {
        Some(value) if !value.is_empty() => value.cow_to_ascii_lowercase(),
        Some(_) => {
            command.callback(&ctx, Vec::new()).await?;

            return Ok(());
        }
        None => return Err(Error::InvalidCommandOptions),
    };

    let name = name.as_ref();
    let badges = ctx.redis().badges().await?;
    let archived_badges = badges.get();
    let mut choices = Vec::with_capacity(25);

    for badge in archived_badges.iter() {
        if badge.name.cow_to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(&badge.name));
        }

        if badge.description.to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(&badge.description));
        }

        if choices.len() >= 25 {
            choices.truncate(25);

            break;
        }
    }

    command.callback(&ctx, choices).await?;

    Ok(())
}

fn new_choice(name: &str) -> CommandOptionChoice {
    CommandOptionChoice::String {
        name: name.to_owned(),
        name_localizations: None,
        value: name.to_owned(),
    }
}

#[derive(Debug)]
struct MatchingString<'s> {
    value: &'s str,
    similarity: f32,
}

impl<'s> MatchingString<'s> {
    fn new(name: &'_ str, value: &'s str) -> Self {
        let lowercase = value.to_ascii_lowercase();

        Self::new_(name, value, lowercase.as_str())
    }

    fn new_with_cow(name: &'_ str, value: &'s str) -> Self {
        let lowercase = value.cow_to_ascii_lowercase();

        Self::new_(name, value, lowercase.as_ref())
    }

    fn new_(name: &'_ str, value: &'s str, lowercase: &'_ str) -> Self {
        let similarity = levenshtein_similarity(name, lowercase);

        Self { value, similarity }
    }
}

impl PartialEq for MatchingString<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.similarity == other.similarity
    }
}

impl Eq for MatchingString<'_> {}

impl PartialOrd for MatchingString<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.similarity.partial_cmp(&other.similarity)
    }
}

impl Ord for MatchingString<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}
