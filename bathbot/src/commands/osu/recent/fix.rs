use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    MessageBuilder,
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
};
use eyre::{Report, Result};
use rand::{Rng, thread_rng};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};

use super::RecentFix;
use crate::{
    commands::osu::{FixEntry, FixScore, require_link, user_not_found},
    core::{Context, commands::CommandOrigin},
    embeds::{EmbedData, FixScoreEmbed},
    manager::redis::osu::{UserArgs, UserArgsError, UserArgsSlim},
    util::osu::IfFc,
};

pub(super) async fn fix(orig: CommandOrigin<'_>, args: RecentFix) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let mode = match args.mode.map(GameMode::from).or(config.mode) {
        None => GameMode::Osu,
        Some(GameMode::Mania) => return orig.error("Can't fix mania scores \\:(").await,
        Some(mode) => mode,
    };

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| {
                    config.score_data.map(ScoreData::is_legacy)
                })
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    // Retrieve the user and their recent scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let scores_fut = Context::osu_scores()
        .recent(legacy_scores)
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
                user.username.as_str(),
            );

            return orig.error(content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or scores");

            return Err(err);
        }
    };

    let num = match args.index.as_deref() {
        Some("random" | "?") => match scores.is_empty() {
            false => thread_rng().gen_range(0..scores.len()),
            true => 0,
        },
        Some(n) => match n.parse::<usize>() {
            Ok(n) => n.saturating_sub(1),
            Err(_) => {
                let content = "Failed to parse index. \
                Must be an integer between 1 and 100 or `random` / `?`.";

                return orig.error(content).await;
            }
        },
        None => 0,
    };

    let scores_len = scores.len();

    let (score, map, top) = match scores.into_iter().nth(num) {
        Some(score) => {
            let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());
            let map_fut = Context::osu_map().map(score.map_id, checksum);

            let user_args = UserArgsSlim::user_id(user.user_id.to_native()).mode(score.mode);
            let best_fut = Context::osu_scores()
                .top(legacy_scores)
                .limit(100)
                .exec(user_args);

            match tokio::join!(map_fut, best_fut) {
                (Ok(map), Ok(best)) => (score, map, best),
                (Err(err), _) => {
                    let _ = orig.error(GENERAL_ISSUE).await;

                    return Err(Report::new(err));
                }
                (_, Err(err)) => {
                    let _ = orig.error(OSU_API_ISSUE).await;
                    let err = Report::new(err).wrap_err("failed to get top scores");

                    return Err(err);
                }
            }
        }
        None => {
            let username = user.username.as_str();

            let content = format!(
                "There {verb} only {num} score{plural} in `{username}`'{genitive} recent history.",
                verb = if scores_len != 1 { "are" } else { "is" },
                num = scores_len,
                plural = if scores_len != 1 { "s" } else { "" },
                genitive = if username.ends_with('s') { "" } else { "s" }
            );

            return orig.error(content).await;
        }
    };

    let pp = match score.pp {
        Some(pp) => pp,
        None => Context::pp(&map).score(&score).performance().await.pp() as f32,
    };

    let score = ScoreSlim::new(score, pp);
    let if_fc = IfFc::new(&score, &map).await;
    let score = Some(FixScore { score, top, if_fc });
    let entry = FixEntry { user, map, score };

    let embed = FixScoreEmbed::new(&entry, None).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}
