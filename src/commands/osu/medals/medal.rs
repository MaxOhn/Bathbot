use crate::{
    embeds::{EmbedData, MedalEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSEKAI_ISSUE},
        levenshtein_similarity, MessageExt,
    },
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
    let medal = match ctx.psql().get_medal_by_name(name).await {
        Ok(Some(medal)) => medal,
        Ok(None) => return no_medal(&ctx, &data, name).await,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let map_fut = ctx.clients.custom.get_osekai_beatmaps(&medal.name);
    let comment_fut = ctx.clients.custom.get_osekai_comments(&medal.name);

    let (mut maps, mut comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(why) => {
            let _ = data.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    comments.retain(|comment| comment.parent_id == 0);
    comments.sort_unstable_by_key(|comment| Reverse(comment.vote_sum));
    maps.sort_unstable_by_key(|map| Reverse(map.vote_sum));

    let embed_data = MedalEmbed::new(medal, None, maps, comments);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.65;

async fn no_medal(ctx: &Context, data: &CommandData<'_>, name: &str) -> BotResult<()> {
    let medals = match ctx.psql().get_medal_names().await {
        Ok(medals) => medals,
        Err(why) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let mut medals: Vec<_> = medals
        .into_iter()
        .map(|medal| {
            let medal = medal.to_ascii_lowercase();

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
