use super::require_link;
use crate::{
    arguments::{Args, MultNameArgs},
    bail,
    embeds::{CommonEmbed, EmbedData},
    pagination::{CommonPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        get_combined_thumbnail, MessageExt,
    },
    BotResult, Context,
};

use futures::future::{try_join_all, TryFutureExt};
use itertools::Itertools;
use rosu::{
    backend::requests::{BeatmapRequest, UserRequest},
    models::{Beatmap, GameMode, Score, User},
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};
use twilight::model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn common_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args = MultNameArgs::new(args, 10);
    let names = match args.names.len() {
        0 => {
            let content = "You need to specify at least one osu username. \
                    If you're not linked, you must specify at least two names.";
            return msg.respond(&ctx, content).await;
        }
        1 => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
                args.names.push(name);
                args.names
            }
            None => {
                let prefix = match msg.guild_id {
                    Some(guild_id) => ctx.config_first_prefix(guild_id),
                    None => "<".to_owned(),
                };
                let content = format!(
                    "Since you're not linked via `{}link`, \
                        you must specify at least two names.",
                    prefix
                );
                return msg.respond(&ctx, content).await;
            }
        },
        _ => args.names,
    };

    // Remove duplicates, hence HashSet
    if names.iter().unique().count() == 1 {
        let content = "Give at least two different names";
        return msg.respond(&ctx, content).await;
    }

    // Retrieve all users
    let user_futs = names
        .iter()
        .enumerate()
        .map(|(i, name)| ctx.osu_user(&name, mode).map_ok(move |user| (i, user)));
    let users: HashMap<u32, User> = match try_join_all(user_futs).await {
        Ok(users) => match users.iter().find(|(_, user)| user.is_none()) {
            Some((idx, _)) => {
                let content = format!("User `{}` was not found", names[*idx]);
                return msg.respond(&ctx, content).await;
            }
            None => users
                .into_iter()
                .filter_map(|(_, user)| user.map(|user| (user.user_id, user)))
                .collect(),
        },
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Retrieve each user's top scores
    let score_futs = users
        .iter()
        .map(|(_, user)| user.get_top_scores(&ctx.clients.osu, 100, mode));
    let mut all_scores = match try_join_all(score_futs).await {
        Ok(all_scores) => all_scores,
        Err(why) => {
            msg.respond(&ctx, OSU_API_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Consider only scores on common maps
    let mut map_ids: HashSet<u32> = all_scores
        .iter()
        .map(|scores| scores.iter().flat_map(|s| s.beatmap_id))
        .flatten()
        .collect();
    map_ids.retain(|&id| {
        all_scores
            .iter()
            .all(|scores| scores.iter().any(|s| s.beatmap_id.unwrap() == id))
    });
    all_scores
        .iter_mut()
        .for_each(|scores| scores.retain(|s| map_ids.contains(&s.beatmap_id.unwrap())));

    // Flatten scores, sort by beatmap id, then group by beatmap id
    let mut all_scores: Vec<Score> = all_scores.into_iter().flatten().collect();
    all_scores.sort_by(|s1, s2| s1.beatmap_id.cmp(&s2.beatmap_id));
    let mut all_scores: HashMap<u32, Vec<Score>> = all_scores
        .into_iter()
        .group_by(|score| score.beatmap_id.unwrap())
        .into_iter()
        .map(|(map_id, scores)| (map_id, scores.collect()))
        .collect();

    // Sort each group by pp value, then take the best 3
    all_scores.iter_mut().for_each(|(_, scores)| {
        scores.sort_by(|s1, s2| s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal));
        scores.truncate(3);
    });

    // Consider only the top 10 maps with the highest avg pp among the users
    let mut pp_avg: Vec<(u32, f32)> = all_scores
        .iter()
        .map(|(&map_id, scores)| {
            let sum = scores.iter().fold(0.0, |sum, next| sum + next.pp.unwrap());
            (map_id, sum / scores.len() as f32)
        })
        .collect();
    pp_avg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Try retrieving all maps of common scores from the database
    let mut maps = {
        let map_id_vec = map_ids.iter().copied().collect_vec();
        match ctx.clients.psql.get_beatmaps(&map_id_vec).await {
            Ok(maps) => maps,
            Err(why) => {
                warn!("Error while getting maps from DB: {}", why);
                HashMap::default()
            }
        }
    };
    let amount_common = map_ids.len();
    map_ids.retain(|id| !maps.contains_key(id));

    // Retrieve all missing maps from the API
    let missing_maps: Option<Vec<_>> = if map_ids.is_empty() {
        None
    } else {
        let map_futs = map_ids.into_iter().map(|id| {
            BeatmapRequest::new()
                .map_id(id)
                .queue_single(&ctx.clients.osu)
                .map_ok(move |map| (id, map))
        });
        match try_join_all(map_futs).await {
            Ok(maps_result) => match maps_result.iter().find(|(_, map)| map.is_none()) {
                Some((id, _)) => {
                    let content = format!("API returned no result for map id {}", id);
                    return msg.respond(&ctx, content).await;
                }
                None => {
                    let maps = maps_result
                        .into_iter()
                        .map(|(id, map)| {
                            let map = map.unwrap();
                            maps.insert(id, map.clone());
                            map
                        })
                        .collect();
                    Some(maps)
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };

    // Accumulate all necessary data
    let len = names.iter().map(|name| name.len() + 4).sum();
    let mut content = String::with_capacity(len);
    let mut iter = names.into_iter();
    let _ = write!(content, "`{}`", iter.next().unwrap());
    for name in iter {
        let _ = write!(content, ", `{}`", name);
    }
    if amount_common == 0 {
        content.push_str(" have no common scores");
    } else {
        let _ = write!(
            content,
            " have {} common beatmap{} in their top 100",
            amount_common,
            if amount_common > 1 { "s" } else { "" }
        );
    }

    // Keys have no strict order, hence inconsistent result
    let user_ids: Vec<u32> = users.keys().copied().collect();
    let thumbnail = match get_combined_thumbnail(&user_ids).await {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            warn!("Error while combining avatars: {}", why);
            None
        }
    };
    let id_pps = &pp_avg[..10.min(pp_avg.len())];
    let data = CommonEmbed::new(&users, &all_scores, &maps, id_pps, 0);

    // Creating the embed
    let embed = data.build().build();
    let m = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?;
    let response = if let Some(thumbnail) = thumbnail {
        m.attachment("avatar_fuse.png", thumbnail).await?
    } else {
        m.await?
    };

    // Add missing maps to database
    if let Some(maps) = missing_maps {
        let len = maps.len();
        match ctx.clients.psql.insert_beatmaps(&maps).await {
            Ok(_) if len == 1 => {}
            Ok(_) => info!("Added {} maps to DB", len),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if pp_avg.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = CommonPagination::new(response, users, all_scores, maps, pp_avg);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Compare maps of players' top 100s")]
#[long_desc(
    "Compare the users' top 100 and check which \
     maps appear in each top list (up to 10 users)"
)]
#[usage("[name1] [name2] ...")]
#[example("badewanne3 \"nathan on osu\" idke")]
pub async fn common(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top 100s")]
#[long_desc(
    "Compare the mania users' top 100 and check which \
     maps appear in each top list (up to 10 users)"
)]
#[usage("[name1] [name2] ...")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonm")]
pub async fn commonmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top 100s")]
#[long_desc(
    "Compare the taiko users' top 100 and check which \
     maps appear in each top list (up to 10 users)"
)]
#[usage("[name1] [name2] ...")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commont")]
pub async fn commontaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Compare maps of players' top 100s")]
#[long_desc(
    "Compare the ctb users' top 100 and check which \
     maps appear in each top list (up to 10 users)"
)]
#[usage("[name1] [name2] ...")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonc")]
pub async fn commonctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    common_main(GameMode::CTB, ctx, msg, args).await
}
