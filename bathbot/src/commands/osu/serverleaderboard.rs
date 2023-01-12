use std::sync::Arc;

use bathbot_macros::SlashCommand;
use bathbot_model::{RankingKind, UserModeStatsColumn, UserStatsColumn, UserStatsKind};
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::Result;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    pagination::RankingPagination,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "serverleaderboard",
    dm_permission = false,
    help = "Various osu! leaderboards for linked server members.\n\
    Whenever any command is used that requests an osu! user, the retrieved user will be cached.\n\
    The leaderboards will contain all members of this server that are linked to an osu! username \
    which was cached through some command beforehand.\n\
    Since only the cached data is used, no values are guaranteed to be up-to-date. \
    They're just snapshots from the last time the user was retrieved through a command.\n\n\
    There are three reasons why a user might be missing from the leaderboard:\n\
    - They are not linked through the `/link` command\n\
    - Their osu! user stats have not been cached yet. \
    Try using any command that retrieves the user, e.g. `/profile`, in order to cache them.\n\
    - Members of this server are not stored as such. Maybe let bade know :eyes:"
)]
/// Various osu! leaderboards for linked server members
pub enum ServerLeaderboard {
    #[command(name = "all_modes")]
    AllModes(ServerLeaderboardAllModes),
    #[command(name = "osu")]
    Osu(ServerLeaderboardOsu),
    #[command(name = "taiko")]
    Taiko(ServerLeaderboardTaiko),
    #[command(name = "ctb")]
    Catch(ServerLeaderboardCatch),
    #[command(name = "mania")]
    Mania(ServerLeaderboardMania),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "all_modes")]
/// Various leaderboards across all modes for linked server members
pub struct ServerLeaderboardAllModes {
    #[command(help = "Specify what kind of leaderboard to show.\
    Notably:\n\
    - `Comments`: Considers comments on things like osu! articles or mapsets\n\
    - `Played maps`: Only maps with leaderboards count i.e. ranked, loved, or approved maps")]
    /// Specify what kind of leaderboard to show
    kind: UserStatsColumn,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "osu")]
/// Various osu!standard leaderboards for linked server members
pub struct ServerLeaderboardOsu {
    /// Specify what kind of leaderboard to show
    kind: UserModeStatsColumn,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "taiko")]
/// Various osu!taiko leaderboards for linked server members
pub struct ServerLeaderboardTaiko {
    /// Specify what kind of leaderboard to show
    kind: UserModeStatsColumn,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "ctb")]
/// Various osu!ctb leaderboards for linked server members
pub struct ServerLeaderboardCatch {
    /// Specify what kind of leaderboard to show
    kind: UserModeStatsColumn,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "mania")]
/// Various osu!mania leaderboards for linked server members
pub struct ServerLeaderboardMania {
    /// Specify what kind of leaderboard to show
    kind: UserModeStatsColumn,
}

async fn slash_serverleaderboard(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = ServerLeaderboard::from_interaction(command.input_data())?;

    let owner = command.user_id()?;
    let guild_id = command.guild_id.unwrap(); // command is only processed in guilds

    let members: Vec<_> = ctx.cache.members(guild_id, |id| id.get() as i64);

    let guild_icon = ctx
        .cache
        .guild(guild_id, |g| g.icon().copied())
        .ok()
        .flatten()
        .map(|icon| (guild_id, icon));

    let author_name_fut = ctx.user_config().osu_name(owner);

    let ((author_name_res, entries_res), kind) = match args {
        ServerLeaderboard::AllModes(args) => {
            let entries_fut = ctx.osu_user().stats(&members, args.kind);

            let kind = RankingKind::UserStats {
                guild_icon,
                kind: UserStatsKind::AllModes { column: args.kind },
            };

            (tokio::join!(author_name_fut, entries_fut), kind)
        }
        ServerLeaderboard::Osu(args) => {
            let entries_fut = ctx
                .osu_user()
                .stats_mode(&members, GameMode::Osu, args.kind);

            let kind = RankingKind::UserStats {
                guild_icon,
                kind: UserStatsKind::Mode {
                    mode: GameMode::Osu,
                    column: args.kind,
                },
            };

            (tokio::join!(author_name_fut, entries_fut), kind)
        }
        ServerLeaderboard::Taiko(args) => {
            let entries_fut = ctx
                .osu_user()
                .stats_mode(&members, GameMode::Taiko, args.kind);

            let kind = RankingKind::UserStats {
                guild_icon,
                kind: UserStatsKind::Mode {
                    mode: GameMode::Taiko,
                    column: args.kind,
                },
            };

            (tokio::join!(author_name_fut, entries_fut), kind)
        }
        ServerLeaderboard::Catch(args) => {
            let entries_fut = ctx
                .osu_user()
                .stats_mode(&members, GameMode::Catch, args.kind);

            let kind = RankingKind::UserStats {
                guild_icon,
                kind: UserStatsKind::Mode {
                    mode: GameMode::Catch,
                    column: args.kind,
                },
            };

            (tokio::join!(author_name_fut, entries_fut), kind)
        }
        ServerLeaderboard::Mania(args) => {
            let entries_fut = ctx
                .osu_user()
                .stats_mode(&members, GameMode::Mania, args.kind);

            let kind = RankingKind::UserStats {
                guild_icon,
                kind: UserStatsKind::Mode {
                    mode: GameMode::Mania,
                    column: args.kind,
                },
            };

            (tokio::join!(author_name_fut, entries_fut), kind)
        }
    };

    let entries = match entries_res {
        Ok(entries) => entries,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let author_name = match author_name_res {
        Ok(name_opt) => name_opt,
        Err(err) => {
            warn!("{err:?}");

            None
        }
    };

    if entries.is_empty() {
        let content = "No user data found for members of this server :(\n\
            There could be three reasons:\n\
            - Members of this server are not linked through the `/link` command\n\
            - Their osu! user stats have not been cached yet. \
            Try using any command that retrieves an osu! user, e.g. `/profile`, in order to cache them.\n\
            - Members of this server are not stored as such. Maybe let bade know :eyes:";

        command.error(&ctx, content).await?;

        return Ok(());
    }

    let author_idx = author_name.and_then(|name| entries.name_pos(&name));
    let total = entries.len();
    let builder = RankingPagination::builder(entries, total, author_idx, kind);

    builder
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
