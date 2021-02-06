use crate::{
    arguments::{try_link_name, Args},
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_tracking,
    unwind_error,
    util::{constants::OSU_API_ISSUE, matcher, numbers, MessageExt},
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

async fn mapper_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    mapper: Option<String>,
    args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let mut args = args.map(|arg| try_link_name(&ctx, Some(arg)).unwrap());
    let first;
    let user;

    let mapper = if let Some(mapper) = mapper {
        match args.next() {
            Some(arg) => user = arg.to_lowercase(),
            None => match ctx.get_link(msg.author.id.0) {
                Some(name) => user = name,
                None => return super::require_link(&ctx, msg).await,
            },
        }

        mapper
    } else {
        match args.next() {
            Some(arg) => first = arg,
            None => {
                let content = "You need to specify at least one osu username for the mapper. \
                If you're not linked, you must specify at least two names.";

                return msg.error(&ctx, content).await;
            }
        }

        match args.next() {
            Some(arg) if !matches!(arg.as_str(), "-c" | "-convert" | "-converts") => {
                user = first;
                arg.to_lowercase()
            }
            _ => match ctx.get_link(msg.author.id.0) {
                Some(name) => {
                    user = name;
                    first.to_lowercase()
                }
                None => {
                    let prefix = ctx.config_first_prefix(msg.guild_id);

                    let content = format!(
                        "Since you're not linked via `{}link`, \
                        you must specify at least two names.",
                        prefix
                    );

                    return msg.error(&ctx, content).await;
                }
            },
        }
    };

    let filter_converts = matches!(
        args.next().as_deref(),
        Some("-c") | Some("-convert") | Some("-converts")
    );

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(user.as_str()).mode(mode);
    let scores_fut = ctx.osu().top_scores(user.as_str()).mode(mode).limit(100);
    let join_result = tokio::try_join!(user_fut, scores_fut);

    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", user);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    let mut maps = HashMap::new();
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores
        .iter()
        .enumerate()
        .filter_map(|(_, s)| s.beatmap_id)
        .collect();

    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            unwind_error!(warn, why, "Error while getting maps from DB: {}");

            HashMap::default()
        }
    };
    debug!("Found {}/{} beatmaps in DB", maps.len(), scores.len());

    let retrieving_msg = if scores.len() - maps.len() > 10 {
        let content = format!(
            "Retrieving {} maps from the api...",
            scores.len() - maps.len()
        );

        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .await
            .ok()
    } else {
        None
    };

    // Retrieving all missing beatmaps
    let mut scores_data = Vec::with_capacity(scores.len());
    let mut missing_maps = Vec::new();

    for (i, score) in scores.into_iter().enumerate() {
        let map_id = score.beatmap_id.unwrap();

        let map = if let Some(map) = maps.remove(&map_id) {
            map
        } else {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(Some(map)) => {
                    missing_maps.push(map.clone());

                    map
                }
                Ok(None) => {
                    let content = format!("The API returned no beatmap for map id {}", map_id);

                    return msg.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        };

        scores_data.push((i + 1, score, map));
    }

    scores_data.retain(|(_, _, map)| {
        // Filter converts
        if filter_converts && map.mode != mode {
            return false;
        }

        // Either the version contains the mapper name (guest diff'd by mapper)
        // or the map is created by mapper name and not guest diff'd by someone else
        let version = map.version.to_lowercase();

        version.contains(&mapper)
            || (map.creator.to_lowercase() == mapper && !matcher::is_guest_diff(&version))
    });

    // Accumulate all necessary data
    let content = match mapper.as_str() {
        "sotarks" => {
            let amount = scores_data.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = user.username
            );

            let to_push = match amount {
                0 => "proud of you \\:)",
                1..=5 => "that's already too many...",
                6..=10 => "kinda sad \\:/",
                11..=15 => "pretty sad \\:(",
                16..=25 => "this is so sad \\:((",
                26..=35 => "you need to stop this",
                36..=49 => "you have a serious problem...",
                50 => "that's half. HALF.",
                51..=79 => "how do you sleep at night...",
                80..=89 => "so close to ultimate disaster...",
                90..=99 => "i'm not even mad, that's just impressive",
                100 => "you did it. \"Congrats\".",
                _ => "wait how did you do that",
            };

            content.push_str(to_push);

            content
        }
        _ => format!(
            "{} of `{}`'{} top score maps were mapped by `{}`",
            scores_data.len(),
            user.username,
            if user.username.ends_with('s') {
                ""
            } else {
                "s"
            },
            mapper
        ),
    };

    let pages = numbers::div_euclid(5, scores_data.len());
    let data = TopEmbed::new(&user, scores_data.iter().take(5), mode, (1, pages)).await;

    if let Some(msg) = retrieving_msg {
        let _ = ctx.http.delete_message(msg.channel_id, msg.id).await;
    }

    // Creating the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Add missing maps to database
    if !missing_maps.is_empty() {
        match ctx.psql().insert_beatmaps(&missing_maps).await {
            Ok(n) if n < 2 => {}
            Ok(n) => info!("Added {} maps to DB", n),
            Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = TopPagination::new(response, user, scores_data, mode);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mapper): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("How many maps of a user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
pub async fn mapper(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::STD, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a mania user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a mania user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native mania maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mapperm")]
pub async fn mappermania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::MNA, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native taiko maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mappert")]
pub async fn mappertaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::TKO, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native ctb maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[aliases("mapperc")]
async fn mapperctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::CTB, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a user's top100 are made by Sotarks?")]
#[long_desc(
    "How many maps of a user's top100 are made by Sotarks?\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username]")]
#[example("badewanne3")]
pub async fn sotarks(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mapper = Some(String::from("sotarks"));
    mapper_main(GameMode::STD, ctx, msg, mapper, args).await
}
