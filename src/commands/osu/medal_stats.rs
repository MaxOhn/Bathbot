use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, MedalStatsEmbed},
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Display medal stats for a user")]
#[usage("[username]")]
#[example("badewanne3", r#""im a fancy lad""#)]
#[aliases("ms")]
async fn medalstats(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let user = match ctx.osu().user(name.as_str()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let profile_fut = ctx
        .clients
        .custom
        .get_osu_profile(user.user_id, GameMode::STD, true);
    let (profile, medals) = match profile_fut.await {
        Ok(tuple) => tuple,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why.into());
        }
    };
    let embed = MedalStatsEmbed::new(profile, medals).build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
