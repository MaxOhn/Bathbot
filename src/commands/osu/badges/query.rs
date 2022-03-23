use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    fmt::Write,
    sync::Arc,
};

use eyre::Report;
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{ApplicationCommand, ApplicationCommandAutocomplete},
    },
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};

use crate::{
    core::Context,
    embeds::{BadgeEmbed, EmbedData},
    error::Error,
    pagination::{BadgePagination, Pagination},
    util::{
        constants::OSEKAI_ISSUE, get_combined_thumbnail, levenshtein_similarity, numbers, CowUtils,
        InteractionExt, MessageBuilder, MessageExt,
    },
    BotResult,
};

use super::BadgeOrder;

pub(super) async fn query_(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    name: String,
    sort_by: BadgeOrder,
) -> BotResult<()> {
    let mut badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let name_ = name.cow_to_ascii_lowercase();
    let name = name_.as_ref();
    let mut found_exact = false;

    badges.retain(|badge| {
        if found_exact {
            false
        } else {
            let lowercase_name = badge.name.cow_to_ascii_lowercase();
            let lowercase_desc = badge.description.to_ascii_lowercase();

            if lowercase_name == name || lowercase_desc == name {
                found_exact = true;

                true
            } else {
                lowercase_name.contains(name) || lowercase_desc.contains(name)
            }
        }
    });

    if found_exact && badges.len() > 1 {
        let len = badges.len();
        badges.swap(0, len - 1);
        badges.truncate(1);
    }

    sort_by.apply(&mut badges);

    let owners = if let Some(badge) = badges.first() {
        let owners_fut = ctx.clients.custom.get_osekai_badge_owners(badge.badge_id);

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

    let pages = numbers::div_euclid(1, badges.len());

    let embed = BadgeEmbed::new(&badges[0], &owners, (1, pages))
        .into_builder()
        .build();

    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = bytes {
        builder = builder.file("badge_owners.png", bytes);
    }

    let response_raw = command.create_message(&ctx, builder).await?;

    if badges.len() == 1 {
        return Ok(());
    }

    let response = response_raw.model().await?;
    let mut owners_map = BTreeMap::new();
    owners_map.insert(0, owners);

    let pagination = BadgePagination::new(response, badges, owners_map, Arc::clone(&ctx));
    let owner = command.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn no_badge_found(ctx: &Context, command: &ApplicationCommand, name: &str) -> BotResult<()> {
    let badges = match ctx.redis().badges().await {
        Ok(badges) => badges,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let mut heap = BinaryHeap::with_capacity(2 * badges.len());

    for badge in &badges {
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

    command.error(ctx, content).await
}

pub async fn handle_autocomplete(
    ctx: Arc<Context>,
    command: ApplicationCommandAutocomplete,
) -> BotResult<()> {
    let mut name = None;

    if let Some(option) = command.data.options.first() {
        match option
            .options
            .first()
            .and_then(|option| option.value.as_ref())
        {
            Some(value) => name = Some(value),
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let name_ = match name {
        Some(name) if !name.is_empty() => name.cow_to_ascii_lowercase(),
        _ => return respond_autocomplete(&ctx, &command, Vec::new()).await,
    };

    let name = name_.as_ref();
    let badges = ctx.redis().badges().await?;
    let mut choices = Vec::with_capacity(25);

    for badge in badges {
        if badge.name.cow_to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(badge.name));
        }

        if badge.description.to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(badge.description));
        }

        if choices.len() >= 25 {
            choices.truncate(25);

            break;
        }
    }

    respond_autocomplete(&ctx, &command, choices).await
}

fn new_choice(name: String) -> CommandOptionChoice {
    CommandOptionChoice::String {
        value: name.clone(),
        name,
    }
}

async fn respond_autocomplete(
    ctx: &Context,
    command: &ApplicationCommandAutocomplete,
    choices: Vec<CommandOptionChoice>,
) -> BotResult<()> {
    let data = InteractionResponseData {
        choices: Some(choices),
        ..Default::default()
    };

    let response = InteractionResponse {
        kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
        data: Some(data),
    };

    ctx.interaction()
        .create_response(command.id, &command.token, &response)
        .exec()
        .await?;

    Ok(())
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
