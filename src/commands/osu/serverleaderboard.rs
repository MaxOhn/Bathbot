use std::sync::Arc;

use eyre::Report;
use rosu_v2::prelude::GameMode;
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    database::UserStatsColumn,
    embeds::{EmbedData, RankingEmbed, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{
        constants::{
            common_literals::{ACCURACY, CTB, MANIA, OSU, TAIKO},
            GENERAL_ISSUE,
        },
        numbers, ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, Context, Error,
};

pub async fn slash_serverleaderboard(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    let kind = UserStatsColumn::slash(&mut command)?;
    let owner = command.user_id()?;
    let guild_id = command.guild_id.unwrap(); // command is only processed in guilds

    let members = ctx.cache.members(guild_id, |id| id.get() as i64);

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
        Err(why) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
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
    let builder = embed_data.into_builder().build().into();
    let response_raw = command.create_message(&ctx, builder).await?;

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

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

impl UserStatsColumn {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(_) => return Err(Error::InvalidCommandOptions),
                CommandOptionValue::Integer(_) => return Err(Error::InvalidCommandOptions),
                CommandOptionValue::Boolean(_) => return Err(Error::InvalidCommandOptions),
                CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                    "all_modes" => kind = Some(Self::parse_all_modes_subcommand(options)?),
                    OSU | TAIKO | CTB | MANIA => {
                        let mode = match option.name.as_str() {
                            OSU => GameMode::STD,
                            TAIKO => GameMode::TKO,
                            CTB => GameMode::CTB,
                            MANIA => GameMode::MNA,
                            _ => unreachable!(),
                        };

                        kind = Some(Self::parse_mode_subcommand(mode, options)?);
                    }
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }

    fn parse_all_modes_subcommand(options: Vec<CommandDataOption>) -> BotResult<Self> {
        let mut kind = None;

        for option in options {
            kind = match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    "type" => match value.as_str() {
                        "badges" => Some(UserStatsColumn::Badges),
                        "comments" => Some(UserStatsColumn::Comments),
                        "followers" => Some(UserStatsColumn::Followers),
                        "forum_posts" => Some(UserStatsColumn::ForumPosts),
                        "graveyard_mapsets" => Some(UserStatsColumn::GraveyardMapsets),
                        "join_date" => Some(UserStatsColumn::JoinDate),
                        "loved_mapsets" => Some(UserStatsColumn::LovedMapsets),
                        "mapping_followers" => Some(UserStatsColumn::MappingFollowers),
                        "medals" => Some(UserStatsColumn::Medals),
                        "namechanges" => Some(UserStatsColumn::Usernames),
                        "played_maps" => Some(UserStatsColumn::PlayedMaps),
                        "ranked_mapsets" => Some(UserStatsColumn::RankedMapsets),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            };
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }

    fn parse_mode_subcommand(mode: GameMode, options: Vec<CommandDataOption>) -> BotResult<Self> {
        let mut kind = None;

        for option in options {
            kind = match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    "type" => match value.as_str() {
                        ACCURACY => Some(UserStatsColumn::Accuracy { mode }),
                        "avg_hits" => Some(UserStatsColumn::AverageHits { mode }),
                        "count_ssh" => Some(UserStatsColumn::CountSsh { mode }),
                        "count_ss" => Some(UserStatsColumn::CountSs { mode }),
                        "count_sh" => Some(UserStatsColumn::CountSh { mode }),
                        "count_s" => Some(UserStatsColumn::CountS { mode }),
                        "count_a" => Some(UserStatsColumn::CountA { mode }),
                        "level" => Some(UserStatsColumn::Level { mode }),
                        "max_combo" => Some(UserStatsColumn::MaxCombo { mode }),
                        "playcount" => Some(UserStatsColumn::Playcount { mode }),
                        "playtime" => Some(UserStatsColumn::Playtime { mode }),
                        "pp" => Some(UserStatsColumn::Pp { mode }),
                        "rank_country" => Some(UserStatsColumn::RankCountry { mode }),
                        "rank_global" => Some(UserStatsColumn::RankGlobal { mode }),
                        "replays" => Some(UserStatsColumn::Replays { mode }),
                        "score_ranked" => Some(UserStatsColumn::ScoreRanked { mode }),
                        "score_total" => Some(UserStatsColumn::ScoreTotal { mode }),
                        "scores_first" => Some(UserStatsColumn::ScoresFirst { mode }),
                        "total_hits" => Some(UserStatsColumn::TotalHits { mode }),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            };
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

const SPECIFY_KIND: &str = "Specify what kind of leaderboard to show";

fn mode_option() -> Vec<MyCommandOption> {
    let choices = vec![
        CommandOptionChoice::String {
            name: "Accuracy".to_owned(),
            value: ACCURACY.to_owned(),
        },
        CommandOptionChoice::String {
            name: "Average hits per play".to_owned(),
            value: "avg_hits".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Count SSH".to_owned(),
            value: "count_ssh".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Count SS".to_owned(),
            value: "count_ss".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Count SH".to_owned(),
            value: "count_sh".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Count S".to_owned(),
            value: "count_s".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Count A".to_owned(),
            value: "count_a".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Country rank".to_owned(),
            value: "rank_country".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Global numbers 1s".to_owned(),
            value: "scores_first".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Global rank".to_owned(),
            value: "rank_global".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Level".to_owned(),
            value: "level".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Max combo".to_owned(),
            value: "max_combo".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Playcount".to_owned(),
            value: "playcount".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Playtime".to_owned(),
            value: "playtime".to_owned(),
        },
        CommandOptionChoice::String {
            name: "PP".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Ranked score".to_owned(),
            value: "score_ranked".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Replays watched".to_owned(),
            value: "replays".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Total hits".to_owned(),
            value: "total_hits".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Total score".to_owned(),
            value: "score_total".to_owned(),
        },
    ];

    let kind = MyCommandOption::builder("type", SPECIFY_KIND).string(choices, true);

    vec![kind]
}

fn all_modes_option() -> Vec<MyCommandOption> {
    let choices = vec![
        CommandOptionChoice::String {
            name: "Badges".to_owned(),
            value: "badges".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Comments".to_owned(),
            value: "comments".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Followers".to_owned(),
            value: "followers".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Forum posts".to_owned(),
            value: "forum_posts".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Graveyard mapsets".to_owned(),
            value: "graveyard_mapsets".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Join date".to_owned(),
            value: "join_date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Loved mapsets".to_owned(),
            value: "loved_mapsets".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Mapping followers".to_owned(),
            value: "mapping_followers".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Medals".to_owned(),
            value: "medals".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Namechanges".to_owned(),
            value: "namechanges".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Played maps".to_owned(),
            value: "played_maps".to_owned(),
        },
        CommandOptionChoice::String {
            name: "Ranked mapsets".to_owned(),
            value: "ranked_mapsets".to_owned(),
        },
    ];

    let help = "Specify what kind of leaderboard to show.\
    Notably:\n\
    - `Comments`: Considers comments on things like osu! articles or mapsets\n\
    - `Played maps`: Only maps with leaderboards count i.e. ranked, loved, or approved maps";

    let kind = MyCommandOption::builder("type", SPECIFY_KIND)
        .help(help)
        .string(choices, true);

    vec![kind]
}

pub fn define_serverleaderboard() -> MyCommand {
    let all_modes_description = "Various leaderboards across all modes for linked server members";
    let all_modes =
        MyCommandOption::builder("all_modes", all_modes_description).subcommand(all_modes_option());

    let osu_description = "Various osu!standard leaderboards for linked server members";
    let osu = MyCommandOption::builder(OSU, osu_description).subcommand(mode_option());

    let taiko_description = "Various osu!taiko leaderboards for linked server members";
    let taiko = MyCommandOption::builder(TAIKO, taiko_description).subcommand(mode_option());

    let ctb_description = "Various osu!ctb leaderboards for linked server members";
    let ctb = MyCommandOption::builder(CTB, ctb_description).subcommand(mode_option());

    let mania_description = "Various osu!mania leaderboards for linked server members";
    let mania = MyCommandOption::builder(MANIA, mania_description).subcommand(mode_option());

    let options = vec![all_modes, osu, taiko, ctb, mania];
    let description = "Various osu! leaderboards for linked server members.";

    let help = "Various osu! leaderboards for linked server members.\n\
        Whenever any command is used that requests an osu! user, the retrieved user will be cached.\n\
        The leaderboards will contain all members of this server that are linked to an osu! username \
        which was cached through some command beforehand.\n\
        Since only the cached data is used, no values are guaranteed to be up-to-date. \
        They're just snapshots from the last time the user was retrieved through a command.\n\n\
        There are three reasons why a user might be missing from the leaderboard:\n\
        - They are not linked through the `/link` command\n\
        - Their osu! user stats have not been cached yet. \
        Try using any command that retrieves the user, e.g. `/profile`, in order to cache them.\n\
        - Members of this server are not stored as such. Maybe let bade know :eyes:";

    MyCommand::new("serverleaderboard", description)
        .help(help)
        .options(options)
}
