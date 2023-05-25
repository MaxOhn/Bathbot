use std::{cmp::Ordering, collections::BTreeMap, fmt::Write, sync::Arc};

use bathbot_model::OsekaiBadge;
use bathbot_util::{
    constants::{AVATAR_URL, OSEKAI_ISSUE},
    string_cmp::levenshtein_similarity,
    CowUtils,
};
use eyre::{Result, WrapErr};
use rkyv::{Deserialize, Infallible};
use twilight_interactions::command::AutocompleteValue;
use twilight_model::application::command::{CommandOptionChoice, CommandOptionChoiceValue};

use super::BadgesQuery_;
use crate::{
    active::{impls::BadgesPagination, ActiveMessages},
    core::Context,
    manager::redis::RedisData,
    util::{
        interaction::InteractionCommand, osu::get_combined_thumbnail, Authored,
        InteractionCommandExt,
    },
};

pub(super) async fn query(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: BadgesQuery_,
) -> Result<()> {
    let BadgesQuery_ { name, sort } = args;

    let name = match name {
        AutocompleteValue::None => return handle_autocomplete(&ctx, &command, String::new()).await,
        AutocompleteValue::Focused(name) => return handle_autocomplete(&ctx, &command, name).await,
        AutocompleteValue::Completed(name) => name,
    };

    let badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached badges"));
        }
    };

    let name_ = name.cow_to_ascii_lowercase();
    let name = name_.as_ref();
    let mut found_exact = false;

    let mut badges: Vec<OsekaiBadge> = match badges {
        RedisData::Original(badges) => badges
            .into_iter()
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
            .flatten()
            .collect(),
        RedisData::Archive(badges) => badges
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
            .collect(),
    };

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

                return Err(err.wrap_err("Failed to get badge owners"));
            }
        }
    } else {
        return no_badge_found(&ctx, &command, name).await;
    };

    let urls: Vec<_> = owners
        .iter()
        .map(|owner| format!("{AVATAR_URL}{}", owner.user_id).into_boxed_str())
        .collect();

    let urls = urls.iter().map(Box::as_ref);

    let bytes = if badges.len() == 1 {
        match get_combined_thumbnail(&ctx, urls, owners.len() as u32, Some(1024)).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                warn!(?err, "Failed to combine avatars");

                None
            }
        }
    } else {
        None
    };

    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners.into_boxed_slice());

    let pagination = BadgesPagination::builder()
        .badges(badges.into_boxed_slice())
        .owners(owners_map)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(bytes.map(|bytes| ("badge_owners.png".to_owned(), bytes)))
        .begin(ctx, &mut command)
        .await
}

async fn no_badge_found(ctx: &Context, command: &InteractionCommand, name: &str) -> Result<()> {
    let badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached badges"));
        }
    };

    let mut list = match &badges {
        RedisData::Original(badges) => {
            let mut list = Vec::with_capacity(2 * badges.len());

            for badge in badges.iter() {
                list.push(MatchingString::new_with_cow(name, &badge.name));
                list.push(MatchingString::new(name, &badge.description));
            }

            list
        }
        RedisData::Archive(badges) => {
            let mut list = Vec::with_capacity(2 * badges.len());

            for badge in badges.iter() {
                list.push(MatchingString::new_with_cow(name, &badge.name));
                list.push(MatchingString::new(name, &badge.description));
            }

            list
        }
    };

    list.sort_unstable_by(|a, b| b.cmp(a));

    let mut content = format!("No badge found that matches `{name}`.");
    let mut list = list.into_iter();

    if let Some(matching) = list.next().filter(|m| m.similarity > 0.0) {
        let _ = write!(content, " Did you mean `{}`", matching.value);
        let mut i = 1;

        for matching in list.filter(|m| m.similarity > 0.0) {
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
    ctx: &Context,
    command: &InteractionCommand,
    name: String,
) -> Result<()> {
    let name = if name.is_empty() {
        command.autocomplete(ctx, Vec::new()).await?;

        return Ok(());
    } else {
        name.cow_to_ascii_lowercase()
    };

    let name = name.as_ref();

    let badges = ctx
        .redis()
        .badges()
        .await
        .wrap_err("failed to get cached badges")?;

    let mut choices = Vec::with_capacity(25);

    match badges {
        RedisData::Original(badges) => {
            for badge in badges.iter() {
                if badge.name.cow_to_ascii_lowercase().contains(name) {
                    if let Some(choice) = new_choice(&badge.name) {
                        choices.push(choice);
                    }
                }

                if badge.description.to_ascii_lowercase().contains(name) {
                    if let Some(choice) = new_choice(&badge.description) {
                        choices.push(choice);
                    }
                }

                if choices.len() >= 25 {
                    choices.truncate(25);

                    break;
                }
            }
        }
        RedisData::Archive(badges) => {
            for badge in badges.iter() {
                if badge.name.cow_to_ascii_lowercase().contains(name) {
                    if let Some(choice) = new_choice(&badge.name) {
                        choices.push(choice);
                    }
                }

                if badge.description.to_ascii_lowercase().contains(name) {
                    if let Some(choice) = new_choice(&badge.description) {
                        choices.push(choice);
                    }
                }

                if choices.len() >= 25 {
                    choices.truncate(25);

                    break;
                }
            }
        }
    }

    command.autocomplete(ctx, choices).await?;

    Ok(())
}

fn new_choice(name: &str) -> Option<CommandOptionChoice> {
    (name.len() <= 100).then(|| CommandOptionChoice {
        name: name.to_owned(),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(name.to_owned()),
    })
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
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.similarity == other.similarity
    }
}

impl Eq for MatchingString<'_> {}

impl PartialOrd for MatchingString<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.similarity.partial_cmp(&other.similarity)
    }
}

impl Ord for MatchingString<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}
