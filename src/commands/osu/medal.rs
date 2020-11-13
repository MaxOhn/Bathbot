use crate::{
    arguments::Args,
    embeds::{EmbedData, MedalEmbed},
    util::MessageExt,
    BotResult, Context,
};

use std::sync::Arc;
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
async fn medal(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let name = match args.next() {
        Some(name) => name,
        None => {
            let content = "You must specify a medal name.";
            return msg.error(&ctx, content).await;
        }
    };
    let medal = match ctx.clients.custom.get_osekai_medal(name).await {
        Ok(Some(medal)) => medal,
        Ok(None) => {
            let content = format!("No medal found with the name `{}`", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let content = "Some issue with the osekai api, blame bade";
            let _ = msg.error(&ctx, content).await;
            return Err(why.into());
        }
    };
    let embed = MedalEmbed::new(medal).build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
