use crate::{
    commands::osu::get_osekai_medals,
    custom_client::OsekaiMedal,
    embeds::{EmbedData, MedalEmbed},
    util::{constants::OSEKAI_ISSUE, levenshtein_similarity, CowUtils, MessageExt},
    BotResult, CommandData, Context,
};

use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    sync::Arc,
};

#[command]
#[short_desc("Display info about an osu! medal")]
#[long_desc(
    "Display info about an osu! medal.\n\
    The given name must be exact (but case-insensitive).\n\
    All data originates from [osekai](https://osekai.net/medals/), \
    check it out for more info."
)]
#[usage("[medal name]")]
#[example(r#""50,000 plays""#, "any%")]
async fn medal(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, args, num } => {
            let name = args.rest().trim_matches('"');

            if name.is_empty() {
                return msg.error(&ctx, "You must specify a medal name.").await;
            }

            _medal(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => super::slash_medal(ctx, *command).await,
    }
}

pub(super) async fn _medal(ctx: Arc<Context>, data: CommandData<'_>, name: &str) -> BotResult<()> {
    let mut medals = match get_osekai_medals(&ctx).await {
        Ok(medals) => medals,
        Err(err) => {
            let _ = data.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let name = name.cow_to_ascii_lowercase();

    let medal = match medals
        .iter()
        .position(|m| m.name.to_ascii_lowercase() == name)
    {
        Some(idx) => medals.swap_remove(idx),
        None => return no_medal(&ctx, &data, name.as_ref(), medals).await,
    };

    let map_fut = ctx.clients.custom.get_osekai_beatmaps(&medal.name);
    let comment_fut = ctx.clients.custom.get_osekai_comments(&medal.name);

    let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(why) => {
            let _ = data.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let top_comment = comments
        .into_iter()
        .filter(|comment| comment.parent_id == 0)
        .max_by_key(|comment| comment.vote_sum)
        .filter(|comment| comment.vote_sum > 0);

    maps.sort_unstable_by_key(|map| Reverse(map.vote_sum));

    let embed_data = MedalEmbed::new(medal, None, maps, top_comment);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.6;

async fn no_medal(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    medals: Vec<OsekaiMedal>,
) -> BotResult<()> {
    let mut medals: Vec<_> = medals
        .into_iter()
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

    data.error(ctx, content).await
}
