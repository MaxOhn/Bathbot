use std::sync::Arc;

use rosu_v2::prelude::{Beatmap, GameMode, OsuError, Score};

use crate::{
    commands::osu::{get_user_and_scores, prepare_score, unchoke_pp, ScoreArgs, UserArgs},
    core::{commands::CommandOrigin, Context},
    embeds::{EmbedData, FixScoreEmbed},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    },
    BotResult,
};

use super::RecentFix;

pub(super) async fn fix(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RecentFix,
) -> BotResult<()> {
    let (name, mode) = name_mode!(ctx, orig, args);

    if mode == GameMode::MNA {
        return orig.error(&ctx, "Can't fix mania scores \\:(").await;
    }

    // Retrieve the user and their recent scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::recent(100).include_fails(true);

    let (mut user, scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{name}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
            );

            return orig.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    let num = args.index.unwrap_or(1).saturating_sub(1);
    let scores_len = scores.len();

    let (mut score, map, user, mut scores) = match scores.into_iter().nth(num) {
        Some(mut score) => {
            let mapset_fut = ctx
                .psql()
                .get_beatmapset(score.map.as_ref().unwrap().mapset_id);

            let best_fut = ctx
                .osu()
                .user_scores(score.user_id)
                .mode(mode)
                .limit(100)
                .best();

            let user_fut = ctx.osu().user(score.user_id).mode(mode);
            let score_fut = prepare_score(&ctx, &mut score);

            match tokio::join!(mapset_fut, score_fut, user_fut, best_fut) {
                (_, Err(err), ..) | (.., Err(err), _) | (.., Err(err)) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                    return Err(err.into());
                }
                (Ok(mapset), Ok(_), Ok(user), Ok(best)) => {
                    let mut map = score.map.take().unwrap();
                    map.mapset = Some(mapset);

                    (score, map, user, best)
                }
                (Err(_), Ok(_), Ok(user), Ok(best)) => {
                    let mut map = score.map.take().unwrap();

                    let mapset = match ctx.osu().beatmapset(map.mapset_id).await {
                        Ok(mapset) => mapset,
                        Err(err) => {
                            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                            return Err(err.into());
                        }
                    };

                    map.mapset = Some(mapset);

                    (score, map, user, best)
                }
            }
        }
        None => {
            let content = format!(
                "There {verb} only {num} score{plural} in `{name}`'{genitive} recent history.",
                verb = if scores_len != 1 { "are" } else { "is" },
                num = scores_len,
                plural = if scores_len != 1 { "s" } else { "" },
                name = name,
                genitive = if name.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        }
    };

    let unchoked_pp = if score.pp.is_some() && !needs_unchoking(&score, &map) {
        None
    } else {
        match unchoke_pp(&ctx, &mut score, &map).await {
            Ok(pp) => pp,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        }
    };

    // Process tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let gb = ctx.map_garbage_collector(&map);

    let embed_data = FixScoreEmbed::new(user, map, Some((score, scores)), unchoked_pp, None);
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    // Set map on garbage collection list if unranked
    gb.execute(&ctx);

    Ok(())
}

fn needs_unchoking(score: &Score, map: &Beatmap) -> bool {
    match map.mode {
        GameMode::STD => {
            score.statistics.count_miss > 0
                || score.max_combo < map.max_combo.map_or(0, |c| c.saturating_sub(5))
        }
        GameMode::TKO => score.statistics.count_miss > 0,
        GameMode::CTB => score.max_combo != map.max_combo.unwrap_or(0),
        GameMode::MNA => panic!("can not unchoke mania scores"),
    }
}
