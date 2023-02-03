use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use bathbot_macros::SlashCommand;
use bathbot_model::{HlVersion, RankingEntries, RankingEntry, RankingKind};
use bathbot_util::{constants::GENERAL_ISSUE, IntHasher, MessageBuilder};
use eyre::{ContextCompat, Result, WrapErr};
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::Id;

use crate::{
    commands::GameModeOption,
    games::hl::{GameState, HlComponents},
    pagination::RankingPagination,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt, MessageExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "higherlower")]
/// Play a game of osu! themed higher lower
pub enum HigherLower {
    #[command(name = "pp")]
    ScorePp(HigherLowerScorePp),
    #[command(name = "farm")]
    FarmMaps(HigherLowerFarmMaps),
    #[command(name = "leaderboard")]
    Leaderboard(HigherLowerLeaderboard),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "pp",
    help = "Is the score's pp value higher or lower?\n\
    The players are chosen randomly from the top 5,000 and the top score \
    is chosen randomly as well but the higher the current score is, the more \
    likely it is that the next pp value is close to the previous pp."
)]
/// Is the score's pp value higher or lower?
pub struct HigherLowerScorePp {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "farm",
    help = "Is the amount of times the map appears in top scores higher or lower?\n\
    All counts are provided by [osutracker](https://osutracker.com) which only includes a portion \
    of the actual data but it should be representative, at least for >300pp scores.\n\
    The maps are chosen randomly based on [this weight function](https://www.desmos.com/calculator/u4jt9t4jnj)."
)]
/// Is the amount of times the map appears in top scores higher or lower?
pub struct HigherLowerFarmMaps;

#[derive(CommandModel, CreateCommand)]
#[command(name = "leaderboard")]
/// Get the server leaderboard for higherlower highscores
pub struct HigherLowerLeaderboard {
    /// Specify the version to get the highscores of
    version: HlVersion,
}

async fn slash_higherlower(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = HigherLower::from_interaction(command.input_data())?;

    if let HigherLower::Leaderboard(ref args) = args {
        return higherlower_leaderboard(ctx, command, args.version).await;
    }

    let user = command.user_id()?;

    if let Some(game) = ctx.hl_games().lock(&user).await.remove() {
        let components = HlComponents::disabled();
        let builder = MessageBuilder::new().components(components);

        (game.msg, game.channel)
            .update(&ctx, &builder, command.permissions)
            .wrap_err("lacking permission to update message")?
            .await
            .wrap_err("failed to remove components of previous game")?;
    }

    let game_res = match args {
        HigherLower::ScorePp(args) => {
            let mode = match args.mode.map(GameMode::from) {
                Some(mode) => mode,
                None => ctx.user_config().mode(user).await?.unwrap_or(GameMode::Osu),
            };

            GameState::score_pp(&ctx, &command, mode).await
        }
        HigherLower::FarmMaps(_) => GameState::farm_maps(&ctx, &command).await,
        HigherLower::Leaderboard(_) => unreachable!(),
    };

    let mut game = match game_res {
        Ok(game) => game,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let embed = game.make_embed().await;
    let components = HlComponents::higherlower();
    let builder = MessageBuilder::new().embed(embed).components(components);

    let response = command.update(&ctx, &builder).await?.model().await?;

    game.msg = response.id;
    ctx.hl_games().own(user).await.insert(game);

    Ok(())
}

async fn higherlower_leaderboard(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    version: HlVersion,
) -> Result<()> {
    let guild = match command.guild_id {
        Some(guild) => guild,
        None => {
            let content = "That command is only available in servers";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let mut scores = match ctx.games().higherlower_leaderboard(version).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let members: HashSet<_, IntHasher> = ctx.cache.members(guild, |id| id.get() as i64);
    scores.retain(|row| members.contains(&row.discord_id));

    let author = command.user_id()?.get() as i64;

    scores.sort_unstable_by(|a, b| b.highscore.cmp(&a.highscore));
    let author_idx = scores.iter().position(|row| row.discord_id == author);

    // Gather usernames for initial page
    let mut entries = BTreeMap::new();

    for (i, row) in scores.iter().enumerate().take(20) {
        let id = Id::new(row.discord_id as u64);

        let name = match ctx.user_config().osu_name(id).await {
            Ok(Some(name)) => name,
            Ok(None) => ctx
                .cache
                .user(id, |user| user.name.as_str().into())
                .unwrap_or_else(|_| "<unknown user>".into()),
            Err(err) => {
                warn!("{err:?}");

                "<unknown user>".into()
            }
        };

        let entry = RankingEntry {
            country: None,
            name,
            value: row.highscore as u64,
        };

        entries.insert(i, entry);
    }

    let entries = RankingEntries::Amount(entries);
    let total = scores.len();
    let data = RankingKind::HlScores { scores, version };

    RankingPagination::builder(entries, total, author_idx, data)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
