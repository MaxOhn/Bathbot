use std::sync::Arc;

use chrono::{Duration, Utc};
use eyre::Report;
use rosu_v2::prelude::{GameMode, OsuError, User};
use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
};

use crate::{
    commands::{
        osu::{get_user, get_user_and_scores, ScoreArgs, UserArgs},
        parse_mode_option, MyCommand, MyCommandOption,
    },
    core::{commands::CommandData, Context},
    database::UserConfig,
    embeds::{EmbedData, GraphEmbed},
    error::Error,
    util::{
        constants::{common_literals::MODE, GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        InteractionExt, MessageBuilder, MessageExt,
    },
    BotResult,
};

use super::{option_discord, option_mode, option_name};

async fn graph(ctx: Arc<Context>, data: CommandData<'_>, args: GraphArgs) -> BotResult<()> {
    let GraphArgs { config, kind } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), mode);

    let tuple_option = match kind {
        GraphKind::MedalProgression => medals_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::PlaycountReplays => {
            playcount_replays_graph(&ctx, &data, &name, &user_args).await?
        }
        GraphKind::RankProgression => rank_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::ScoreTime => score_time_graph(&ctx, &data, &name, user_args).await?,
        GraphKind::Sniped => sniped_graph(&ctx, &data, &name, &user_args).await?,
        GraphKind::SnipeCount => snipe_count_graph(&ctx, &data, &name, &user_args).await?,
    };

    let (user, graph) = match tuple_option {
        Some(tuple) => tuple,
        None => return Ok(()),
    };

    let embed = GraphEmbed::new(&user).into_builder().build();
    let builder = MessageBuilder::new().embed(embed).file("graph.png", &graph);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

async fn medals_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    if let Some(ref mut medals) = user.medals {
        medals.sort_unstable_by_key(|medal| medal.achieved_at);
    }

    let bytes = match super::medals::stats::graph(user.medals.as_ref().unwrap()) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have any medals");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn playcount_replays_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let mut user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let bytes = match super::profile::graphs(ctx, &mut user).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content = format!("`{name}` does not have enough playcount data points");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn rank_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let _user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    todo!()
}

async fn score_time_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let score_args = ScoreArgs::top(100);

    let (_user, _scores) = match get_user_and_scores(ctx, user_args, &score_args).await {
        Ok(tuple) => tuple,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    todo!()
}

async fn sniped_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let (sniper, snipee) = if ctx.contains_country(user.country_code.as_str()) {
        let now = Utc::now();
        let sniper_fut =
            ctx.clients
                .custom
                .get_national_snipes(&user, true, now - Duration::weeks(8), now);
        let snipee_fut =
            ctx.clients
                .custom
                .get_national_snipes(&user, false, now - Duration::weeks(8), now);

        match tokio::try_join!(sniper_fut, snipee_fut) {
            Ok((mut sniper, snipee)) => {
                sniper.retain(|score| score.sniped.is_some());

                (sniper, snipee)
            }
            Err(err) => {
                let _ = data.error(ctx, HUISMETBENEN_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        data.error(ctx, content).await?;

        return Ok(None);
    };

    let bytes = match super::snipe::sniped::graphs(user.username.as_str(), &sniper, &snipee) {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            let content =
                format!("`{name}` was neither sniped nor sniped other people in the last 8 weeks");
            let builder = MessageBuilder::new().embed(content);
            data.create_message(ctx, builder).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

async fn snipe_count_graph(
    ctx: &Context,
    data: &CommandData<'_>,
    name: &str,
    user_args: &UserArgs<'_>,
) -> BotResult<Option<(User, Vec<u8>)>> {
    let user = match get_user(ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{name}`");
            data.error(ctx, content).await?;

            return Ok(None);
        }
        Err(err) => {
            let _ = data.error(ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let player = if ctx.contains_country(user.country_code.as_str()) {
        let player_fut = ctx
            .clients
            .custom
            .get_snipe_player(&user.country_code, user.user_id);

        match player_fut.await {
            Ok(counts) => counts,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to retrieve snipe player");
                warn!("{report:?}");
                let content = format!("`{name}` has never had any national #1s");
                let builder = MessageBuilder::new().embed(content);
                data.create_message(&ctx, builder).await?;

                return Ok(None);
            }
        }
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        data.error(&ctx, content).await?;

        return Ok(None);
    };

    let graph_result = super::snipe::player_snipe_stats::graphs(
        &player.count_first_history,
        &player.count_sr_spread,
    );

    let bytes = match graph_result {
        Ok(graph) => graph,
        Err(err) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;
            warn!("{:?}", Report::new(err));

            return Ok(None);
        }
    };

    Ok(Some((user, bytes)))
}

struct GraphArgs {
    config: UserConfig,
    kind: GraphKind,
}

enum GraphKind {
    MedalProgression,
    PlaycountReplays,
    RankProgression,
    ScoreTime,
    Sniped,
    SnipeCount,
}

pub async fn slash_graph(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let (subcommand, options) = command
        .data
        .options
        .pop()
        .and_then(|option| match option.value {
            CommandOptionValue::SubCommand(options) => Some((option.name, options)),
            _ => None,
        })
        .ok_or(Error::InvalidCommandOptions)?;

    let mut config = ctx.user_config(command.user_id()?).await?;

    let kind = match subcommand.as_str() {
        "medals" => GraphKind::MedalProgression,
        "playcount_replays" => GraphKind::PlaycountReplays,
        "rank" => {
            for option in options {
                match option.value {
                    CommandOptionValue::String(value) => match option.name.as_str() {
                        MODE => config.mode = parse_mode_option(&value),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                }
            }

            GraphKind::RankProgression
        }
        "score_time" => {
            for option in options {
                match option.value {
                    CommandOptionValue::String(value) => match option.name.as_str() {
                        MODE => config.mode = parse_mode_option(&value),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                }
            }

            GraphKind::ScoreTime
        }
        "sniped" => GraphKind::Sniped,
        "snipe_count" => GraphKind::SnipeCount,
        _ => return Err(Error::InvalidCommandOptions),
    };

    graph(ctx, command.into(), GraphArgs { config, kind }).await
}

pub fn _define_graph() -> MyCommand {
    let medals = MyCommandOption::builder("medals", "Display a user's medal progress over time")
        .subcommand(vec![option_name(), option_discord()]);

    let playcount_replays_description = "Display a user's playcount and replays watched over time";

    let playcount_replays =
        MyCommandOption::builder("playcount_replays", playcount_replays_description)
            .subcommand(vec![option_name(), option_discord()]);

    let rank = MyCommandOption::builder("rank", "Display a user's rank progression over time")
        .subcommand(vec![option_mode(), option_name(), option_discord()]);

    let score_time_description = "Display at what times a user set their top scores";

    let score_time = MyCommandOption::builder("score_time", score_time_description)
        .subcommand(vec![option_mode(), option_name(), option_discord()]);

    let sniped = MyCommandOption::builder("sniped", "Display sniped users of the past 8 weeks")
        .subcommand(vec![option_name(), option_discord()]);

    let snipe_count_description = "Display how a user's national #1 count progressed";

    let snipe_count = MyCommandOption::builder("snipe_count", snipe_count_description)
        .subcommand(vec![option_name(), option_discord()]);

    let subcommands = vec![
        medals,
        playcount_replays,
        rank,
        score_time,
        sniped,
        snipe_count,
    ];

    MyCommand::new("graph", "Display graphs about some data").options(subcommands)
}
