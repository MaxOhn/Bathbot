use std::collections::{BTreeMap, HashSet};

use bathbot_macros::SlashCommand;
use bathbot_model::{
    command_fields::GameModeOption, HlVersion, RankingEntries, RankingEntry, RankingKind,
};
use bathbot_util::{constants::GENERAL_ISSUE, IntHasher};
use eyre::Result;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::Id;

use crate::{
    active::{
        impls::{HigherLowerGame, RankingPagination},
        ActiveMessages,
    },
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "higherlower", desc = "Play a game of osu! themed higher lower")]
pub enum HigherLower {
    #[command(name = "pp")]
    ScorePp(HigherLowerScorePp),
    #[command(name = "leaderboard")]
    Leaderboard(HigherLowerLeaderboard),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "pp",
    desc = "Is the score's pp value higher or lower?",
    help = "Is the score's pp value higher or lower?\n\
    The players are chosen randomly from the top 5,000 and the top score \
    is chosen randomly as well but the higher the current score is, the more \
    likely it is that the next pp value is close to the previous pp."
)]
pub struct HigherLowerScorePp {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "leaderboard",
    desc = "Get the server leaderboard for higherlower highscores"
)]
pub struct HigherLowerLeaderboard;

async fn slash_higherlower(mut command: InteractionCommand) -> Result<()> {
    let args = HigherLower::from_interaction(command.input_data())?;
    let user = command.user_id()?;

    let game_res = match args {
        HigherLower::ScorePp(args) => {
            let mode = match args.mode.map(GameMode::from) {
                Some(mode) => mode,
                None => Context::user_config()
                    .mode(user)
                    .await?
                    .unwrap_or(GameMode::Osu),
            };

            HigherLowerGame::new_score_pp(mode, user).await
        }
        HigherLower::Leaderboard(_) => {
            return higherlower_leaderboard(command, HlVersion::ScorePp).await
        }
    };

    match game_res {
        Ok(game) => {
            ActiveMessages::builder(game)
                .start_by_update(true)
                .begin(&mut command)
                .await
        }
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            Err(err)
        }
    }
}

async fn higherlower_leaderboard(
    mut command: InteractionCommand,
    version: HlVersion,
) -> Result<()> {
    let guild = match command.guild_id {
        Some(guild) => guild,
        None => {
            let content = "That command is only available in servers";
            command.error(content).await?;

            return Ok(());
        }
    };

    let mut scores = match Context::games().higherlower_leaderboard(version).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let members: HashSet<_, IntHasher> = Context::cache()
        .member_ids(guild)
        .await?
        .into_iter()
        .map(|id| id.get() as i64)
        .collect();

    scores.retain(|row| members.contains(&row.discord_id));

    let owner = command.user_id()?;
    let author = owner.get() as i64;

    scores.sort_unstable_by(|a, b| b.highscore.cmp(&a.highscore));
    let author_idx = scores.iter().position(|row| row.discord_id == author);

    // Gather usernames for initial page
    let mut entries = BTreeMap::new();

    for (i, row) in scores.iter().enumerate().take(20) {
        let id = Id::new(row.discord_id as u64);

        let name_opt = match Context::user_config().osu_name(id).await {
            Ok(Some(name)) => Some(name),
            Ok(None) => match Context::cache().user(id).await {
                Ok(Some(user)) => Some(user.name.as_ref().into()),
                Ok(None) => None,
                Err(err) => {
                    warn!("{err:?}");

                    None
                }
            },
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let name = name_opt.unwrap_or_else(|| "<unknown user>".into());

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

    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total)
        .author_idx(author_idx)
        .kind(data)
        .defer(false)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}
