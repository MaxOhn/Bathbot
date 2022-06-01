use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    sync::Arc,
};

use command_macros::command;
use rkyv::{Deserialize, Infallible};
use twilight_model::application::{
    command::CommandOptionChoice, interaction::ApplicationCommandAutocomplete,
};

use crate::{
    core::{commands::CommandOrigin, ArchivedBytes},
    custom_client::OsekaiMedal,
    embeds::MedalEmbed,
    error::Error,
    util::{
        builder::MessageBuilder, constants::OSEKAI_ISSUE, levenshtein_similarity, AutocompleteExt,
        ChannelExt, CowUtils,
    },
    BotResult, Context,
};

use super::MedalInfo;

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
async fn prefix_medal(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    let name = args.rest().trim_matches('"');

    if name.is_empty() {
        msg.error(&ctx, "You must specify a medal name").await?;

        return Ok(());
    }

    info(ctx, msg.into(), MedalInfo { name: name.into() }).await
}

pub(super) async fn info(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalInfo<'_>,
) -> BotResult<()> {
    let MedalInfo { name } = args;

    let medals = match ctx.redis().medals().await {
        Ok(medals) => medals,
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let name = name.cow_to_ascii_lowercase();
    let archived_medals = medals.get();

    let medal: OsekaiMedal = match archived_medals
        .iter()
        .position(|m| m.name.to_ascii_lowercase() == name)
    {
        Some(idx) => archived_medals[idx].deserialize(&mut Infallible).unwrap(),
        None => return no_medal(&ctx, &orig, name.as_ref(), medals).await,
    };

    let map_fut = ctx.client().get_osekai_beatmaps(&medal.name);
    let comment_fut = ctx.client().get_osekai_comments(&medal.name);

    let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let top_comment = comments
        .into_iter()
        .filter(|comment| comment.parent_id == 0)
        .max_by_key(|comment| comment.vote_sum)
        .filter(|comment| comment.vote_sum > 0);

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
    medals: ArchivedBytes<Vec<OsekaiMedal>>,
) -> BotResult<()> {
    let mut medals: Vec<_> = medals
        .get()
        .iter()
        .map(|medal| {
            let medal = medal.name.to_ascii_lowercase();

            (levenshtein_similarity(name, &medal), medal)
        })
        .collect();

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
    let medals = ctx.redis().medals().await?;
    let archived_medals = medals.get();
    let mut choices = Vec::with_capacity(25);

    for medal in archived_medals.iter() {
        if medal.name.to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(&medal.name));

            if choices.len() == 25 {
                break;
            }
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
