use std::{fmt::Write, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{
    GameMode, GameMods, OsuError,
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
    database::{EmbedsSize, MinimizedPp, UserConfig},
    embeds::{EmbedData, PinnedEmbed, TopSingleEmbed},
    error::Error,
    pagination::{Pagination, PinnedPagination},
    util::{
        constants::{
            common_literals::{ACC, ACCURACY, COMBO, DISCORD, MODE, MODS, NAME, SORT},
            OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::{ModSelection, ScoreOrder},
        ApplicationCommandExt, FilterCriteria, InteractionExt, MessageExt, Searchable,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use super::{
    get_user_cached, option_discord, option_mode, option_mods_explicit, option_name, option_query,
    prepare_scores,
};

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
                let scores_fut = ctx
                    .osu()
                    .user_scores(user_args.name)
                    .pinned()
                    .mode(mode)
                    .limit(100);

                prepare_scores(&ctx, scores_fut)
                    .await
                    .map(|scores| (user, scores))
            }
            Err(OsuError::NotFound) => {
                user_args.name = &alt_name;

                let user_fut = get_user_cached(&ctx, &user_args);
                let scores_fut = ctx
                    .osu()
                    .user_scores(user_args.name)
                    .pinned()
                    .mode(mode)
                    .limit(100);

                tokio::try_join!(user_fut, prepare_scores(&ctx, scores_fut))
            }
            Err(err) => Err(err),
        }
    } else {
        let user_fut = get_user_cached(&ctx, &user_args);
        let scores_fut = ctx
            .osu()
            .user_scores(user_args.name)
            .pinned()
            .mode(mode)
            .limit(100);

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
    filter_scores(&ctx, &mut scores, &args).await;

    if let [score] = &scores[..] {
        let embeds_size = match (args.config.embeds_size, data.guild_id()) {
            (Some(size), _) => size,
            (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
            (None, None) => EmbedsSize::default(),
        };

        let minimized_pp = match (args.config.minimized_pp, data.guild_id()) {
            (Some(pp), _) => pp,
            (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
            (None, None) => MinimizedPp::default(),
        };

        let content = write_content(name, &args, 1);
        single_embed(ctx, data, user, score, embeds_size, minimized_pp, content).await?;
    } else {
        let content = write_content(name, &args, scores.len());
        let sort_by = args.sort_by.unwrap_or(ScoreOrder::Pp); // TopOrder::Pp does not show anything
        paginated_embed(ctx, data, user, scores, sort_by, content).await?;
    }

    Ok(())
}

async fn filter_scores(ctx: &Context, scores: &mut Vec<Score>, args: &PinnedArgs) {
    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);

        scores.retain(|score| score.matches(&criteria));
    }

    match args.mods {
        Some(ModSelection::Include(GameMods::NoMod)) => {
            scores.retain(|score| score.mods.is_empty())
        }
        Some(ModSelection::Include(mods)) => {
            scores.retain(|score| score.mods.intersection(mods) == mods)
        }
        Some(ModSelection::Exact(mods)) => scores.retain(|score| score.mods == mods),
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores.retain(|score| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|score| score.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    if let Some(sort_by) = args.sort_by {
        sort_by.apply(ctx, scores).await;
    }
}

async fn single_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    score: &Score,
    embeds_size: EmbedsSize,
    minimized_pp: MinimizedPp,
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

    let embed_data =
        TopSingleEmbed::new(&user, score, personal_idx, global_idx, minimized_pp, &ctx).await?;

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let mut builder = MessageBuilder::new().embed(embed_data.into_builder().build());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            data.create_message(&ctx, builder).await?;
        }
        EmbedsSize::InitialMaximized => {
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
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let mut builder = MessageBuilder::new().embed(embed_data.as_builder().build());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            data.create_message(&ctx, builder).await?;
        }
    }

    Ok(())
}

async fn paginated_embed(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    user: User,
    scores: Vec<Score>,
    sort_by: ScoreOrder,
    content: Option<String>,
) -> BotResult<()> {
    let pages = numbers::div_euclid(5, scores.len());
    let embed_data =
        PinnedEmbed::new(&user, scores.iter().take(5), &ctx, sort_by, (1, pages)).await;
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
    let pagination = PinnedPagination::new(response, user, scores, sort_by, Arc::clone(&ctx));
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
    pub sort_by: Option<ScoreOrder>,
    query: Option<String>,
    mods: Option<ModSelection>,
}

impl PinnedArgs {
    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

    async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut sort_by = None;
        let mut query = None;
        let mut mods = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
                    SORT => match value.as_str() {
                        ACC => sort_by = Some(ScoreOrder::Acc),
                        "bpm" => sort_by = Some(ScoreOrder::Bpm),
                        COMBO => sort_by = Some(ScoreOrder::Combo),
                        "date" => sort_by = Some(ScoreOrder::Date),
                        "len" => sort_by = Some(ScoreOrder::Length),
                        "miss" => sort_by = Some(ScoreOrder::Misses),
                        "pp" => sort_by = Some(ScoreOrder::Pp),
                        "ranked_date" => sort_by = Some(ScoreOrder::RankedDate),
                        "stars" => sort_by = Some(ScoreOrder::Stars),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    "query" => query = Some(value),
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
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
            mods,
        };

        Ok(Ok(args))
    }
}

fn write_content(name: &str, args: &PinnedArgs, amount: usize) -> Option<String> {
    if args.query.is_some() || args.mods.is_some() {
        Some(content_with_condition(args, amount))
    } else if let Some(sort_by) = args.sort_by {
        let genitive = if name.ends_with('s') { "" } else { "s" };

        let content = match sort_by {
            ScoreOrder::Acc => format!("`{name}`'{genitive} pinned scores sorted by accuracy:"),
            ScoreOrder::Bpm => format!("`{name}`'{genitive} pinned scores sorted by BPM:"),
            ScoreOrder::Combo => format!("`{name}`'{genitive} pinned scores sorted by combo:"),
            ScoreOrder::Date => format!("Most recent pinned scores of `{name}`:"),
            ScoreOrder::Length => format!("`{name}`'{genitive} pinned scores sorted by length:"),
            ScoreOrder::Misses => {
                format!("`{name}`'{genitive} pinned scores sorted by miss count:")
            }
            ScoreOrder::Pp => format!("`{name}`'{genitive} pinned scores sorted by pp"),
            ScoreOrder::RankedDate => {
                format!("`{name}`'{genitive} pinned scores sorted by ranked date:")
            }
            ScoreOrder::Stars => format!("`{name}`'{genitive} pinned scores sorted by stars"),
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
        Some(ScoreOrder::Acc) => content.push_str("`Order: Accuracy`"),
        Some(ScoreOrder::Bpm) => content.push_str("`Order: BPM`"),
        Some(ScoreOrder::Combo) => content.push_str("`Order: Combo`"),
        Some(ScoreOrder::Date) => content.push_str("`Order: Date`"),
        Some(ScoreOrder::Length) => content.push_str("`Order: Length`"),
        Some(ScoreOrder::Misses) => content.push_str("`Order: Miss count`"),
        Some(ScoreOrder::Pp) => content.push_str("`Order: Pp`"),
        Some(ScoreOrder::RankedDate) => content.push_str("`Order: Ranked date`"),
        Some(ScoreOrder::Stars) => content.push_str("`Order: Stars`"),
        None => {}
    }

    if let Some(selection) = args.mods {
        if !content.is_empty() {
            content.push_str(" ~ ");
        }

        let (pre, mods) = match selection {
            ModSelection::Include(mods) => ("Include ", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Exact(mods) => ("", mods),
        };

        let _ = write!(content, "`Mods: {pre}{mods}`");
    }

    if let Some(query) = args.query.as_deref() {
        if !content.is_empty() {
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
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: "bpm".to_owned(),
            value: "bpm".to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: "date".to_owned(),
            value: "date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "length".to_owned(),
            value: "len".to_owned(),
        },
        CommandOptionChoice::String {
            name: "map ranked date".to_owned(),
            value: "ranked_date".to_owned(),
        },
        CommandOptionChoice::String {
            name: "misses".to_owned(),
            value: "miss".to_owned(),
        },
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: "stars".to_owned(),
            value: "stars".to_owned(),
        },
    ];

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .string(sort_choices, false);

    let discord = option_discord();
    let query = option_query();

    let mods = option_mods_explicit();

    MyCommand::new("pinned", "Display the user's pinned scores")
        .options(vec![mode, name, sort, query, mods, discord])
}
