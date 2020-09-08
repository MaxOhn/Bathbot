use crate::{
    arguments::Args,
    bail,
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers, MessageExt,
    },
    BotResult, Context,
};

use rosu::{backend::BestRequest, models::GameMode};
use std::{collections::HashMap, sync::Arc};
use twilight::model::channel::Message;

async fn mapper_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    mut mapper: Option<String>,
    mut args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let first;
    let user;
    if mapper.is_none() {
        match args.next() {
            Some(arg) => first = Some(arg.to_lowercase()),
            None => {
                let content = "You need to specify at least one osu username for the mapper. \
                If you're not linked, you must specify at least two names.";
                return msg.error(&ctx, content).await;
            }
        };
        match args.next() {
            Some(arg) => {
                user = first;
                mapper = Some(arg.to_lowercase());
            }
            None => match ctx.get_link(msg.author.id.0) {
                Some(name) => {
                    mapper = first;
                    user = Some(name);
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
    } else {
        match args.next() {
            Some(arg) => user = Some(arg.to_lowercase()),
            None => match ctx.get_link(msg.author.id.0) {
                Some(name) => user = Some(name),
                None => return super::require_link(&ctx, msg).await,
            },
        }
    }
    let name = user.unwrap();
    let mapper = mapper.unwrap();

    // Retrieve the user and their top scores
    let scores_fut = match BestRequest::with_username(&name) {
        Ok(req) => req.mode(mode).limit(100).queue(ctx.osu()),
        Err(_) => {
            let content = format!("Could not build request for osu name `{}`", name);
            return msg.error(&ctx, content).await;
        }
    };
    let join_result = tokio::try_join!(ctx.osu_user(&name, mode), scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Get all relevant maps from the database
    let map_ids: Vec<u32> = scores
        .iter()
        .enumerate()
        .filter_map(|(_, s)| s.beatmap_id)
        .collect();
    let mut maps = match ctx.psql().get_beatmaps(&map_ids).await {
        Ok(maps) => maps,
        Err(why) => {
            warn!("Error while getting maps from DB: {}", why);
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
        let map = if maps.contains_key(&map_id) {
            maps.remove(&map_id).unwrap()
        } else {
            match score.get_beatmap(ctx.osu()).await {
                Ok(map) => {
                    missing_maps.push(map.clone());
                    map
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
                name = name
            );
            let to_push = match amount {
                0 => "proud of you \\:)",
                n if n <= 5 => "that's already too many...",
                n if n <= 10 => "kinda sad \\:/",
                n if n <= 15 => "pretty sad \\:(",
                n if n <= 25 => "this is so sad \\:((",
                n if n <= 30 => "bruuh stop \\:'((",
                n if n <= 35 => "you have a serious problem...",
                n if n >= 80 => "so close to ultimate disaster...",
                n if n >= 90 => "i'm not even mad, that's just impressive",
                50 => "that's half. HALF.",
                100 => "you did it. \"Congrats\".",
                _ => "how do you sleep at night...",
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
    let data = match TopEmbed::new(&ctx, &user, scores_data.iter().take(5), mode, (1, pages)).await
    {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while creating mapper embed: {}", why);
        }
    };

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
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = TopPagination::new(ctx.clone(), response, user, scores_data, mode);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (mapper): {}", why)
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
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
pub async fn mappermania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::MNA, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
pub async fn mappertaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    mapper_main(GameMode::TKO, ctx, msg, None, args).await
}

#[command]
#[short_desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[long_desc(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
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
