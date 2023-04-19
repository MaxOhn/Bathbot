use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    sync::Arc,
};

use bathbot_macros::command;
use bathbot_model::OsekaiMedal;
use bathbot_util::{
    constants::OSEKAI_ISSUE, string_cmp::levenshtein_similarity, CowUtils, MessageBuilder,
};
use eyre::{Result, WrapErr};
use rkyv::{Deserialize, Infallible};
use twilight_interactions::command::AutocompleteValue;
use twilight_model::application::command::{CommandOptionChoice, CommandOptionChoiceValue};

use super::MedalInfo_;
use crate::{
    core::commands::CommandOrigin,
    embeds::MedalEmbed,
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[command]
#[desc("Display info about an osu! medal")]
#[help(
    "Display info about an osu! medal.\n\
    The given name must be exact (but case-insensitive).\n\
    All data originates from [osekai](https://osekai.net/medals/), \
    check it out for more info."
)]
#[usage("[medal name]")]
#[examples(r#""50,000 plays""#, "any%")]
#[group(AllModes)]
async fn prefix_medal(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let name = args.rest().trim_matches('"');

    if name.is_empty() {
        msg.error(&ctx, "You must specify a medal name").await?;

        return Ok(());
    }

    let args = MedalInfo_ {
        name: AutocompleteValue::Completed(name.into()),
    };

    info(ctx, msg.into(), args).await
}

pub(super) async fn info(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalInfo_<'_>,
) -> Result<()> {
    let MedalInfo_ { name } = args;

    let mut medals = match ctx.redis().medals().await {
        Ok(medals) => medals,
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    let name = match (name, &orig) {
        (AutocompleteValue::None, CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(&ctx, command, String::new()).await
        }
        (AutocompleteValue::Focused(name), CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(&ctx, command, name).await
        }
        (AutocompleteValue::Completed(name), _) => name,
        _ => unreachable!(),
    };

    let name = name.cow_to_ascii_lowercase();

    let medal = match medals {
        RedisData::Original(ref mut original) => match original
            .iter()
            .position(|m| m.name.to_ascii_lowercase() == name)
        {
            Some(idx) => original.swap_remove(idx),
            None => return no_medal(&ctx, &orig, name.as_ref(), medals).await,
        },
        RedisData::Archive(ref archived) => {
            match archived
                .iter()
                .position(|m| m.name.to_ascii_lowercase() == name)
            {
                Some(idx) => archived[idx].deserialize(&mut Infallible).unwrap(),
                None => return no_medal(&ctx, &orig, name.as_ref(), medals).await,
            }
        }
    };

    let map_fut = ctx.client().get_osekai_beatmaps(&medal.name);
    let comment_fut = ctx.client().get_osekai_comments(medal.medal_id);

    let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get osekai map or comments"));
        }
    };

    let top_comment = comments
        .into_iter()
        .filter(|comment| comment.parent_id == 0)
        .max_by_key(|comment| comment.vote_sum)
        .filter(|comment| comment.vote_sum > 0);

    // Remove all dups
    maps.sort_unstable_by_key(|map| Reverse(map.map_id));
    maps.dedup_by_key(|map| map.map_id);

    maps.sort_unstable_by_key(|map| Reverse(map.vote_sum));

    let embed_data = MedalEmbed::new(medal, None, maps, top_comment);
    let embed = embed_data.maximized();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.6;

async fn no_medal(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    medals: RedisData<Vec<OsekaiMedal>>,
) -> Result<()> {
    let mut medals: Vec<_> = match medals {
        RedisData::Original(original) => original
            .iter()
            .map(|medal| {
                let medal = medal.name.to_ascii_lowercase();

                (levenshtein_similarity(name, &medal), medal)
            })
            .collect(),
        RedisData::Archive(archived) => archived
            .iter()
            .map(|medal| {
                let medal = medal.name.to_ascii_lowercase();

                (levenshtein_similarity(name, &medal), medal)
            })
            .collect(),
    };

    medals.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    let mut content = format!("No medal found with the name `{name}`.");

    let mut iter = medals
        .into_iter()
        .take(5)
        .take_while(|(similarity, _)| *similarity >= SIMILARITY_THRESHOLD);

    if let Some((_, first)) = iter.next() {
        let _ = write!(content, "\nDid you mean `{first}`");

        for (_, medal) in iter {
            let _ = write!(content, ", `{medal}`");
        }

        content.push('?');
    }

    orig.error(ctx, content).await
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

    let medals = ctx
        .redis()
        .medals()
        .await
        .wrap_err("failed to get cached medals")?;

    let mut choices = Vec::with_capacity(25);

    match medals {
        RedisData::Original(original) => {
            for medal in original.iter() {
                if medal.name.to_ascii_lowercase().starts_with(name) {
                    choices.push(new_choice(&medal.name));

                    if choices.len() == 25 {
                        break;
                    }
                }
            }
        }
        RedisData::Archive(archived) => {
            for medal in archived.iter() {
                if medal.name.to_ascii_lowercase().starts_with(name) {
                    choices.push(new_choice(&medal.name));

                    if choices.len() == 25 {
                        break;
                    }
                }
            }
        }
    }

    command.autocomplete(ctx, choices).await?;

    Ok(())
}

fn new_choice(name: &str) -> CommandOptionChoice {
    CommandOptionChoice {
        name: name.to_owned(),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(name.to_owned()),
    }
}
