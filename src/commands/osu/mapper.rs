use super::{prepare_scores, request_user, ErrorType};
use crate::{
    arguments::{try_link_name, Args},
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers, MessageExt,
    },
    BotResult, Context, Name,
};

use futures::future::TryFutureExt;
use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::channel::Message;

async fn mapper_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    mapper: Option<Name>,
    args: Args<'_>,
) -> BotResult<()> {
    // Parse arguments
    let mut args = args.map(|arg| try_link_name(&ctx, Some(arg)).unwrap());
    let first;
    let user;

    let mapper = if let Some(mapper) = mapper {
        match args.next() {
            Some(arg) => user = arg.to_lowercase().into(),
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

                arg.to_lowercase().into()
            }
            _ => match ctx.get_link(msg.author.id.0) {
                Some(name) => {
                    user = name;

                    first.to_lowercase().into()
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
    let user_fut = request_user(&ctx, &user, Some(mode)).map_err(From::from);
    let scores_fut_1 = ctx
        .osu()
        .user_scores(user.as_str())
        .best()
        .mode(mode)
        .limit(50);

    let scores_fut_2 = ctx
        .osu()
        .user_scores(user.as_str())
        .best()
        .mode(mode)
        .offset(50)
        .limit(50);

    let scores_fut_1 = prepare_scores(&ctx, scores_fut_1);
    let scores_fut_2 = prepare_scores(&ctx, scores_fut_2);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut_1, scores_fut_2) {
        Ok((user, mut scores, mut scores_2)) => {
            scores.append(&mut scores_2);

            (user, scores)
        }
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", user);

            return msg.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    let mut scores: Vec<_> = scores
        .into_iter()
        .enumerate()
        .map(|(i, s)| (i + 1, s))
        .collect();

    scores.retain(|(_, score)| {
        let map = &score.map.as_ref().unwrap();
        let mapset = &score.mapset.as_ref().unwrap();

        // Filter converts
        if filter_converts && map.mode != mode {
            return false;
        }

        // Either the version contains the mapper name (guest diff'd by mapper)
        // or the map is created by mapper name and not guest diff'd by someone else
        let version = map.version.to_lowercase();

        version.contains(mapper.as_str())
            || (mapset.creator_name.to_lowercase().as_str() == mapper.as_str()
                && !matcher::is_guest_diff(&version))
    });

    // Accumulate all necessary data
    let content = match mapper.as_str() {
        "sotarks" => {
            let amount = scores.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = user.username,
            );

            let to_push = match amount {
                0 => "proud of you \\:)",
                1..=4 => "that's already too many...",
                5..=8 => "kinda sad \\:/",
                9..=15 => "pretty sad \\:(",
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
            scores.len(),
            user.username,
            if user.username.ends_with('s') {
                ""
            } else {
                "s"
            },
            mapper
        ),
    };

    let response = if scores.is_empty() {
        msg.respond(&ctx, content).await?
    } else {
        let pages = numbers::div_euclid(5, scores.len());
        let data = TopEmbed::new(&user, scores.iter().take(5), (1, pages)).await;

        // Creating the embed
        ctx.http
            .create_message(msg.channel_id)
            .content(content)?
            .embed(data.into_builder().build())?
            .await?
    };

    // Add maps of scores to DB
    let scores_iter = scores.iter().map(|(_, score)| score);

    if let Err(why) = ctx.psql().store_scores_maps(scores_iter).await {
        unwind_error!(warn, why, "Error while adding score maps to DB: {}")
    }

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = TopPagination::new(response, user, scores);
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
    let mapper = Some("sotarks".into());

    mapper_main(GameMode::STD, ctx, msg, mapper, args).await
}
