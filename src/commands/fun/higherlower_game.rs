use std::{collections::BTreeMap, sync::Arc};

use command_macros::SlashCommand;
use eyre::Report;
use hashbrown::HashSet;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{application::interaction::ApplicationCommand, id::Id};

use crate::{
    commands::{osu::UserValue, GameModeOption},
    embeds::{RankingEntry, RankingKindData},
    games::hl::{GameState, HlComponents, HlVersion},
    pagination::RankingPagination,
    util::{
        builder::MessageBuilder, constants::GENERAL_ISSUE, ApplicationCommandExt, Authored,
        MessageExt,
    },
    BotResult, Context,
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

async fn slash_higherlower(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = HigherLower::from_interaction(command.input_data())?;

    if let HigherLower::Leaderboard(ref args) = args {
        return higherlower_leaderboard(ctx, command, args.version).await;
    }

    let user = command.user_id()?;

    if let Some(game) = ctx.hl_games().lock(&user).await.remove() {
        let components = HlComponents::disabled();
        let builder = MessageBuilder::new().components(components);
        (game.msg, game.channel).update(&ctx, &builder).await?;
    }

    let game_res = match args {
        HigherLower::ScorePp(args) => {
            let mode = match args.mode.map(GameMode::from) {
                Some(mode) => mode,
                None => ctx.user_config(user).await?.mode.unwrap_or(GameMode::Osu),
            };

            GameState::score_pp(&ctx, &*command, mode).await
        }
        HigherLower::FarmMaps(_) => GameState::farm_maps(&ctx, &*command).await,
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
    command: Box<ApplicationCommand>,
    version: HlVersion,
) -> BotResult<()> {
    let guild = match command.guild_id {
        Some(guild) => guild,
        None => {
            let content = "That command is only available in servers";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let mut scores = match ctx.psql().get_higherlower_scores(version).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let members: HashSet<_> = ctx.cache.members(guild, |id| id.get());
    scores.retain(|(id, _)| members.contains(id));

    let author = command.user_id()?;

    scores.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
    let author_idx = scores.iter().position(|(user, _)| *user == author);

    // Gather usernames for initial page
    let mut users = BTreeMap::new();

    for (i, (id, score)) in scores.iter().enumerate().take(20) {
        let id = Id::new(*id);

        let name = match ctx.psql().get_user_osu(id).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => ctx
                .cache
                .user(id, |user| user.name.clone())
                .unwrap_or_else(|_| "Unknown user".to_owned())
                .into(),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to get osu user");
                warn!("{report:?}");

                ctx.cache
                    .user(id, |user| user.name.clone())
                    .unwrap_or_else(|_| "Unknown user".to_owned())
                    .into()
            }
        };

        let entry = RankingEntry {
            value: UserValue::Amount(*score as u64),
            name,
            country: None,
        };

        users.insert(i, entry);
    }

    let total = scores.len();
    let data = RankingKindData::HlScores { scores, version };

    RankingPagination::builder(users, total, author_idx, data)
        .start_by_update()
        .start(ctx, command.into())
        .await
}
