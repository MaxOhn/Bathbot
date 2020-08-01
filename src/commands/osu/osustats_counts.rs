use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, OsuStatsCountsEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context, Error,
};

use futures::future::TryFutureExt;
use rosu::models::GameMode;
use std::sync::Arc;
use twilight::model::channel::Message;

async fn osustats_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let join_result = tokio::try_join!(
        ctx.osu_user(&name, mode).map_err(Error::Osu),
        super::get_globals_count(&ctx, &name, mode)
    );
    let (user, counts) = match join_result {
        Ok((Some(user), counts)) => (user, counts),
        Ok((None, _)) => {
            let content = format!("Could not find user `{}`", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why);
        }
    };
    let data = OsuStatsCountsEmbed::new(user, counts);
    let embed = data.build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("Display in how many top 1-50 map leaderboards the user has a score")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `profile` command."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osc", "osustatscounts")]
pub async fn osustatscount(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display in how many top 1-50 map leaderboards the user has a score")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `profilemania` command."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscm", "osustatscountsmania")]
pub async fn osustatscountmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display in how many top 1-50 map leaderboards the user has a score")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `profiletaiko` command."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osct", "osustatscountstaiko")]
pub async fn osustatscounttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display in how many top 1-50 map leaderboards the user has a score")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `profilectb` command."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscc", "osustatscountsctb")]
pub async fn osustatscountctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::CTB, ctx, msg, args).await
}
