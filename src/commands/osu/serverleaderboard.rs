use std::sync::Arc;

use command_macros::SlashCommand;
use eyre::Report;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    database::UserStatsColumn,
    embeds::{EmbedData, RankingEmbed, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{constants::GENERAL_ISSUE, numbers, ApplicationCommandExt, Authored},
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "serverleaderboard",
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
#[flags(ONLY_GUILDS)]
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
    kind: ServerLeaderboardAllModesKind,
}

#[derive(CommandOption, CreateOption)]
pub enum ServerLeaderboardAllModesKind {
    #[option(name = "Badges", value = "badges")]
    Badges,
    #[option(name = "Comments", value = "comments")]
    Comments,
    #[option(name = "Followers", value = "followers")]
    Followers,
    #[option(name = "Forum posts", value = "forum_posts")]
    ForumPosts,
    #[option(name = "Graveyard mapsets", value = "graveyard_mapsets")]
    GraveyardMapsets,
    #[option(name = "Join date", value = "join_date")]
    JoinDate,
    #[option(name = "Loved mapsets", value = "loved_mapsets")]
    LovedMapsets,
    #[option(name = "Mapping followers", value = "mapping_followers")]
    MappingFollowers,
    #[option(name = "Medals", value = "medals")]
    Medals,
    #[option(name = "Namechanges", value = "namechanges")]
    Namechanges,
    #[option(name = "Played maps", value = "played_maps")]
    PlayedMaps,
    #[option(name = "Ranked mapsets", value = "ranked_mapsets")]
    RankedMapsets,
}

impl From<ServerLeaderboardAllModesKind> for UserStatsColumn {
    fn from(kind: ServerLeaderboardAllModesKind) -> Self {
        match kind {
            ServerLeaderboardAllModesKind::Badges => Self::Badges,
            ServerLeaderboardAllModesKind::Comments => Self::Comments,
            ServerLeaderboardAllModesKind::Followers => Self::Followers,
            ServerLeaderboardAllModesKind::ForumPosts => Self::ForumPosts,
            ServerLeaderboardAllModesKind::GraveyardMapsets => Self::GraveyardMapsets,
            ServerLeaderboardAllModesKind::JoinDate => Self::JoinDate,
            ServerLeaderboardAllModesKind::LovedMapsets => Self::LovedMapsets,
            ServerLeaderboardAllModesKind::MappingFollowers => Self::MappingFollowers,
            ServerLeaderboardAllModesKind::Medals => Self::Medals,
            ServerLeaderboardAllModesKind::Namechanges => Self::Usernames,
            ServerLeaderboardAllModesKind::PlayedMaps => Self::PlayedMaps,
            ServerLeaderboardAllModesKind::RankedMapsets => Self::RankedMapsets,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "osu")]
/// Various osu!standard leaderboards for linked server members
pub struct ServerLeaderboardOsu {
    /// Specify what kind of leaderboard to show
    kind: ServerLeaderboardModeKind,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "taiko")]
/// Various osu!taiko leaderboards for linked server members
pub struct ServerLeaderboardTaiko {
    /// Specify what kind of leaderboard to show
    kind: ServerLeaderboardModeKind,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "ctb")]
/// Various osu!ctb leaderboards for linked server members
pub struct ServerLeaderboardCatch {
    /// Specify what kind of leaderboard to show
    kind: ServerLeaderboardModeKind,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "mania")]
/// Various osu!mania leaderboards for linked server members
pub struct ServerLeaderboardMania {
    /// Specify what kind of leaderboard to show
    kind: ServerLeaderboardModeKind,
}

#[derive(CommandOption, CreateOption)]
pub enum ServerLeaderboardModeKind {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Average hits per play", value = "avg_hits")]
    AvgHits,
    #[option(name = "Count SSH", value = "count_ssh")]
    CountSsh,
    #[option(name = "Count SS", value = "count_ss")]
    CountSs,
    #[option(name = "Count SH", value = "count_sh")]
    CountSh,
    #[option(name = "Count S", value = "count_s")]
    CountS,
    #[option(name = "Count A", value = "count_a")]
    CountA,
    #[option(name = "Country rank", value = "country_rank")]
    CountryRank,
    #[option(name = "Global number 1s", value = "global_firsts")]
    GlobalFirsts,
    #[option(name = "Global rank", value = "global_rank")]
    GlobalRank,
    #[option(name = "Level", value = "level")]
    Level,
    #[option(name = "Max combo", value = "max_combo")]
    MaxCombo,
    #[option(name = "Playcount", value = "playcount")]
    Playcount,
    #[option(name = "Playtime", value = "playtime")]
    Playtime,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Ranked score", value = "ranked_score")]
    RankedScore,
    #[option(name = "Replays watched", value = "replays")]
    ReplaysWatched,
    #[option(name = "Total hits", value = "total_hits")]
    TotalHits,
    #[option(name = "Total score", value = "total_score")]
    TotalScore,
}

impl ServerLeaderboardModeKind {
    fn column(self, mode: GameMode) -> UserStatsColumn {
        match self {
            Self::Acc => UserStatsColumn::Accuracy { mode },
            Self::AvgHits => UserStatsColumn::AverageHits { mode },
            Self::CountSsh => UserStatsColumn::CountSsh { mode },
            Self::CountSs => UserStatsColumn::CountSs { mode },
            Self::CountSh => UserStatsColumn::CountSh { mode },
            Self::CountS => UserStatsColumn::CountS { mode },
            Self::CountA => UserStatsColumn::CountA { mode },
            Self::CountryRank => UserStatsColumn::RankCountry { mode },
            Self::GlobalFirsts => UserStatsColumn::ScoresFirst { mode },
            Self::GlobalRank => UserStatsColumn::RankGlobal { mode },
            Self::Level => UserStatsColumn::Level { mode },
            Self::MaxCombo => UserStatsColumn::MaxCombo { mode },
            Self::Playcount => UserStatsColumn::Playcount { mode },
            Self::Playtime => UserStatsColumn::Playtime { mode },
            Self::Pp => UserStatsColumn::Pp { mode },
            Self::RankedScore => UserStatsColumn::ScoreRanked { mode },
            Self::ReplaysWatched => UserStatsColumn::Replays { mode },
            Self::TotalHits => UserStatsColumn::TotalHits { mode },
            Self::TotalScore => UserStatsColumn::ScoreTotal { mode },
        }
    }
}

async fn slash_serverleaderboard(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = ServerLeaderboard::from_interaction(command.input_data())?;

    let kind = match args {
        ServerLeaderboard::AllModes(args) => args.kind.into(),
        ServerLeaderboard::Osu(args) => args.kind.column(GameMode::STD),
        ServerLeaderboard::Taiko(args) => args.kind.column(GameMode::TKO),
        ServerLeaderboard::Catch(args) => args.kind.column(GameMode::CTB),
        ServerLeaderboard::Mania(args) => args.kind.column(GameMode::MNA),
    };

    let owner = command.user_id()?;
    let guild_id = command.guild_id.unwrap(); // command is only processed in guilds

    let members: Vec<_> = ctx.cache.members(guild_id, |id| id.get() as i64);

    let guild_icon = ctx
        .cache
        .guild(guild_id, |g| g.icon().copied())
        .ok()
        .flatten()
        .map(|icon| (guild_id, icon));

    let name = match ctx.user_config(owner).await {
        Ok(config) => config.into_username(),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to retrieve user config");
            warn!("{report:?}");

            None
        }
    };

    let leaderboard = match ctx.psql().get_osu_users_stats(kind, &members).await {
        Ok(values) => values,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let author_idx = name.and_then(|name| {
        leaderboard
            .iter()
            .find(|(_, entry)| entry.name == name)
            .map(|(idx, _)| *idx)
    });

    if leaderboard.is_empty() {
        let content = "No user data found for members of this server :(\n\
            There could be three reasons:\n\
            - Members of this server are not linked through the `/link` command\n\
            - Their osu! user stats have not been cached yet. \
            Try using any command that retrieves an osu! user, e.g. `/profile`, in order to cache them.\n\
            - Members of this server are not stored as such. Maybe let bade know :eyes:";

        command.error(&ctx, content).await?;

        return Ok(());
    }

    let data = RankingKindData::UserStats { guild_icon, kind };
    let total = leaderboard.len();
    let pages = numbers::div_euclid(20, total);

    // Creating the embed
    let embed_data = RankingEmbed::new(&leaderboard, &data, author_idx, (1, pages));
    let builder = embed_data.build().into();
    let response_raw = command.update(&ctx, &builder).await?;

    if total <= 20 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = RankingPagination::new(
        response,
        Arc::clone(&ctx),
        total,
        leaderboard,
        author_idx,
        data,
    );

    pagination.start(ctx, owner, 60);

    Ok(())
}
