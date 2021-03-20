use super::{prepare_scores, ErrorType};
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, RecentListEmbed},
    pagination::{Pagination, RecentListPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    BotResult, Context,
};

use futures::future::TryFutureExt;
use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

async fn recent_list_main(
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

    // Retrieve the user and their recent scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(&name)
        .recent()
        .mode(mode)
        .limit(50)
        .include_fails(true);

    let scores_fut = prepare_scores(&ctx, scores_fut);

    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
                name
            );

            return msg.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let pages = numbers::div_euclid(10, scores.len());
    let scores_iter = scores.iter().take(10);

    let data = match RecentListEmbed::new(&user, scores_iter, (1, pages)).await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let embed = data.build_owned().build()?;
    let response = msg.respond_embed(&ctx, embed).await?;

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = RecentListPagination::new(Arc::clone(&ctx), response, user, scores);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (recentlist): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a list of a user's most recent plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rl")]
pub async fn recentlist(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent mania plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlm")]
pub async fn recentlistmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent taiko plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlt")]
pub async fn recentlisttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a list of a user's most recent ctb plays")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlc")]
pub async fn recentlistctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_list_main(GameMode::CTB, ctx, msg, args).await
}
