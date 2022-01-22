use std::{fmt::Write, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{
    Beatmap, BeatmapsetCompact, GameMode, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use tokio::time::{sleep, Duration};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
};

use crate::{
    commands::{
        osu::UserArgs, parse_discord, parse_mode_option, DoubleResultCow, MyCommand,
        MyCommandOption,
    },
    database::UserConfig,
    embeds::{EmbedData, PinnedEmbed, TopSingleEmbed},
    error::Error,
    pagination::{Pagination, PinnedPagination},
    util::{
        constants::{
            common_literals::{ACC, ACCURACY, COMBO, DISCORD, MODE, NAME, SORT},
            OSU_API_ISSUE,
        },
        numbers, ApplicationCommandExt, CowUtils, InteractionExt, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::{get_user_cached, option_discord, option_mode, option_name, prepare_scores, TopOrder};

async fn _pinned(ctx: Arc<Context>, data: CommandData<'_>, args: PinnedArgs) -> BotResult<()> {
    let mode = args.config.mode.unwrap_or(GameMode::STD);

    let name = match args.config.username() {
        Some(name) => name.as_str(),
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let mut user_args = UserArgs::new(name, mode);

    let result = if let Some(alt_name) = user_args.whitespaced_name() {
        match get_user_cached(&ctx, &user_args).await {
            Ok(user) => {
                let scores_fut = ctx.osu().user_scores(user_args.name).pinned().limit(100);

                prepare_scores(&ctx, scores_fut)
                    .await
                    .map(|scores| (user, scores))
            }
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;

                let user_fut = get_user_cached(&ctx, &user_args);
                let scores_fut = ctx.osu().user_scores(user_args.name).pinned().limit(100);

                tokio::try_join!(user_fut, prepare_scores(&ctx, scores_fut))
            }
            Err(err) => Err(err),
        }
    } else {
        let user_fut = get_user_cached(&ctx, &user_args);
        let scores_fut = ctx.osu().user_scores(user_args.name).pinned().limit(100);

        tokio::try_join!(user_fut, prepare_scores(&ctx, scores_fut))
    };

    let (mut user, mut scores) = match result {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Filter scores according to query
    filter_scores(&mut scores, &args);

    // Add maps of scores to DB
    let scores_iter = scores.iter();

    // Store maps of scores in DB; combo was inserted earlier
    if let Err(err) = ctx.psql().store_scores_maps(scores_iter).await {
        warn!("{:?}", Report::new(err));
    }

    if let [score] = &scores[..] {
        let maximize = match (args.config.embeds_maximized, data.guild_id()) {
            (Some(embeds_maximized), _) => embeds_maximized,
            (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
            (None, None) => true,
        };

        let content = write_content(name, &args, 1);
        single_embed(ctx, data, user, score, maximize, content).await?;
    } else {
        let content = write_content(name, &args, scores.len());
        paginated_embed(ctx, data, user, scores, content).await?;
    }

    Ok(())
}

fn filter_scores(scores: &mut Vec<Score>, args: &PinnedArgs) {
    if let Some(query) = args.query.as_deref() {
        let needle = query.cow_to_ascii_lowercase();
        let mut haystack = String::new();

        scores.retain(|score| {
            let Beatmap { version, .. } = score.map.as_ref().unwrap();
            let BeatmapsetCompact { artist, title, .. } = score.mapset.as_ref().unwrap();
            haystack.clear();

            let _ = write!(
                haystack,
                "{} - {} [{}]",
                artist.cow_to_ascii_lowercase(),
                title.cow_to_ascii_lowercase(),
                version.cow_to_ascii_lowercase()
            );

            haystack.contains(needle.as_ref())
        });
    }

    if let Some(sort_by) = args.sort_by {
        sort_by.apply(scores);
    }
}

async fn single_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    score: &Score,
    maximize: bool,
    content: Option<String>,
) -> BotResult<()> {
    let map = score.map.as_ref().unwrap();

    // Get indices of score in user top100 and map top50
    let (personal_idx, global_idx) = match map.status {
        Ranked | Loved | Qualified | Approved => {
            let best_fut = ctx
                .osu()
                .user_scores(user.user_id)
                .mode(score.mode)
                .limit(100);

            let global_fut = ctx.osu().beatmap_scores(map.map_id); // TODO: Add .limit(50) when supported by osu!api
            let (best_result, global_result) = tokio::join!(best_fut, global_fut);

            let personal_idx = match best_result {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to get best scores");
                    warn!("{:?}", report);

                    None
                }
            };

            let global_idx = match global_result {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to get global scores");
                    warn!("{:?}", report);

                    None
                }
            };

            (personal_idx, global_idx)
        }
        _ => (None, None),
    };

    let embed_data = TopSingleEmbed::new(&user, score, personal_idx, global_idx).await?;

    // Only maximize if config allows it
    if maximize {
        let mut builder = MessageBuilder::new().embed(embed_data.as_builder().build());

        if let Some(content) = content {
            builder = builder.content(content);
        }

        let response = data.create_message(&ctx, builder).await?.model().await?;

        ctx.store_msg(response.id);

        // Minimize embed after delay
        tokio::spawn(async move {
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let builder = embed_data.into_builder().build().into();

            if let Err(why) = response.update_message(&ctx, builder).await {
                let report = Report::new(why).wrap_err("failed to minimize pinned message");
                warn!("{:?}", report);
            }
        });
    } else {
        let mut builder = MessageBuilder::new().embed(embed_data.as_builder().build());

        if let Some(content) = content {
            builder = builder.content(content);
        }

        data.create_message(&ctx, builder).await?;
    }

    Ok(())
}

async fn paginated_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<Score>,
    content: Option<String>,
) -> BotResult<()> {
    let pages = numbers::div_euclid(5, scores.len());
    let embed_data = PinnedEmbed::new(&user, scores.iter().take(5), (1, pages)).await;
    let embed = embed_data.into_builder().build();

    // Creating the embed
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(content) = content {
        builder = builder.content(content);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = PinnedPagination::new(response, user, scores);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub async fn slash_pinned(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let options = command.yoink_options();

    match PinnedArgs::slash(&ctx, &command, options).await? {
        Ok(args) => _pinned(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

struct PinnedArgs {
    config: UserConfig,
    pub sort_by: Option<TopOrder>,
    query: Option<String>,
}

impl PinnedArgs {
    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut sort_by = None;
        let mut query = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    SORT => match value.as_str() {
                        ACC => sort_by = Some(TopOrder::Acc),
                        COMBO => sort_by = Some(TopOrder::Combo),
                        "date" => sort_by = Some(TopOrder::Date),
                        "len" => sort_by = Some(TopOrder::Length),
                        "miss" => sort_by = Some(TopOrder::Misses),
                        "pp" => sort_by = Some(TopOrder::Position),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "query" => query = Some(value),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let args = Self {
            config,
            sort_by,
            query,
        };

        Ok(Ok(args))
    }
}

fn write_content(name: &str, args: &PinnedArgs, amount: usize) -> Option<String> {
    if args.query.is_some() {
        Some(content_with_condition(args, amount))
    } else if let Some(sort_by) = args.sort_by {
        let genitive = if name.ends_with('s') { "" } else { "s" };

        let content = match sort_by {
            TopOrder::Acc => format!("`{name}`'{genitive} pinned scores sorted by accuracy:"),
            TopOrder::Combo => format!("`{name}`'{genitive} pinned scores sorted by combo:"),
            TopOrder::Date => format!("Most recent pinned scores of `{name}`:"),
            TopOrder::Length => format!("`{name}`'{genitive} pinned scores sorted by length:"),
            TopOrder::Misses => format!("`{name}`'{genitive} pinned scores sorted by miss count:"),
            TopOrder::Position => format!("`{name}`'{genitive} pinned scores sorted by pp"),
        };

        Some(content)
    } else if amount == 0 {
        Some(format!("`{name}` has not pinned any scores"))
    } else if amount == 1 {
        Some(format!("`{name}` has pinned 1 score:"))
    } else {
        None
    }
}

fn content_with_condition(args: &PinnedArgs, amount: usize) -> String {
    let mut content = String::with_capacity(64);

    match args.sort_by {
        Some(TopOrder::Acc) => content.push_str("`Order: Accuracy`"),
        Some(TopOrder::Combo) => content.push_str("`Order: Combo`"),
        Some(TopOrder::Date) => content.push_str("`Order: Date`"),
        Some(TopOrder::Length) => content.push_str("`Order: Length`"),
        Some(TopOrder::Misses) => content.push_str("`Order: Misscount`"),
        Some(TopOrder::Position) => content.push_str("`Order: Pp`"),
        None => {}
    }

    if let Some(query) = args.query.as_deref() {
        if args.sort_by.is_some() {
            content.push_str(" ~ ");
        }

        let _ = write!(content, "`Query: {query}`");
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching pinned score{plural}:");

    content
}

pub fn define_pinned() -> MyCommand {
    let mode = option_mode();
    let name = option_name();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
        CommandOptionChoice::String {
            name: "misses".to_owned(),
            value: "miss".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .string(sort_choices, false);

    let discord = option_discord();

    let query_description = "Search for a specific artist, title, or difficulty name";

    let query_help = "Search for a specific artist, title, or difficulty name.\n\
        Filters out all scores for which `{artist} - {title} [{version}]` does not contain the query.";

    let query = MyCommandOption::builder("query", query_description)
        .help(query_help)
        .string(vec![], false);

    MyCommand::new("pinned", "Display the user's pinned scores")
        .options(vec![mode, name, sort, query, discord])
}
