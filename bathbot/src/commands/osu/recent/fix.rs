use std::sync::Arc;

use bathbot_model::ScoreSlim;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};

use super::RecentFix;
use crate::{
    commands::osu::{user_not_found, FixEntry, FixScore},
    core::{commands::CommandOrigin, Context},
    embeds::{EmbedData, FixScoreEmbed},
    manager::redis::osu::{UserArgs, UserArgsSlim},
    util::osu::IfFc,
};

pub(super) async fn fix(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: RecentFix) -> Result<()> {
    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    if mode == GameMode::Mania {
        return orig.error(&ctx, "Can't fix mania scores \\:(").await;
    }

    // Retrieve the user and their recent scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let scores_fut = ctx
        .osu_scores()
        .recent()
        .limit(100)
        .include_fails(true)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::Osu => "",
                    GameMode::Taiko => "taiko ",
                    GameMode::Catch => "ctb ",
                    GameMode::Mania => "mania ",
                },
                user.username(),
            );

            return orig.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    let num = args.index.unwrap_or(1).saturating_sub(1);
    let scores_len = scores.len();

    let (score, map, top) = match scores.into_iter().nth(num) {
        Some(score) => {
            let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());
            let map_fut = ctx.osu_map().map(score.map_id, checksum);

            let user_args = UserArgsSlim::user_id(user.user_id()).mode(score.mode);
            let best_fut = ctx.osu_scores().top().limit(100).exec(user_args);

            match tokio::join!(map_fut, best_fut) {
                (Ok(map), Ok(best)) => (score, map, best),
                (Err(err), _) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(Report::new(err));
                }
                (_, Err(err)) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                    let err = Report::new(err).wrap_err("failed to get top scores");

                    return Err(err);
                }
            }
        }
        None => {
            let username = user.username();

            let content = format!(
                "There {verb} only {num} score{plural} in `{username}`'{genitive} recent history.",
                verb = if scores_len != 1 { "are" } else { "is" },
                num = scores_len,
                plural = if scores_len != 1 { "s" } else { "" },
                genitive = if username.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
    };

    let pp = match score.pp {
        Some(pp) => pp,
        None => ctx.pp(&map).score(&score).performance().await.pp() as f32,
    };

    let score = ScoreSlim::new(score, pp);
    let if_fc = IfFc::new(&ctx, &score, &map).await;
    let score = Some(FixScore { score, top, if_fc });
    let entry = FixEntry { user, map, score };

    let embed = FixScoreEmbed::new(&entry, None).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}
