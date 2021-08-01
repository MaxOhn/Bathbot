use crate::{
    embeds::{EmbedData, MedalEmbed},
    util::{constants::GENERAL_ISSUE, levenshtein_similarity, MessageExt},
    Args, BotResult, Context,
};

use std::{cmp::Ordering, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

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
async fn medal(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let name = args.rest().trim_matches('"');

    if name.is_empty() {
        let content = "You must specify a medal name.";

        return msg.error(&ctx, content).await;
    }

    let medal = match ctx.clients.custom.get_osekai_medal(name).await {
        Ok(Some(medal)) => medal,
        Ok(None) => return no_medal(&ctx, msg, name).await,
        Err(why) => {
            let content = "Some issue with the osekai api, blame bade";
            let _ = msg.error(&ctx, content).await;

            return Err(why.into());
        }
    };

    let embed = &[MedalEmbed::new(medal).into_builder().build()];
    msg.build_response(&ctx, |m| m.embeds(embed)).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.8;

async fn no_medal(ctx: &Context, msg: &Message, name: &str) -> BotResult<()> {
    let medals = match ctx.psql().get_medals().await {
        Ok(medals) => medals,
        Err(why) => {
            let _ = msg.error(ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let mut medals: Vec<_> = medals
        .into_iter()
        .map(|(_, medal)| {
            let medal = medal.name.to_ascii_lowercase();

            (levenshtein_similarity(name, &medal), medal)
        })
        .collect();

    medals.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    let mut content = format!("No medal found with the name `{}`.", name);

    let mut iter = medals
        .into_iter()
        .take(5)
        .take_while(|(similarity, _)| *similarity >= SIMILARITY_THRESHOLD);

    if let Some((_, first)) = iter.next() {
        let _ = write!(content, "\nDid you mean `{}`", first);

        for (_, medal) in iter {
            let _ = write!(content, ", `{}`", medal);
        }

        content.push('?');
    }

    msg.error(ctx, content).await
}
