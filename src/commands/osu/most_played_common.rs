use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, MostPlayedCommonEmbed},
    pagination::{MostPlayedCommonPagination, Pagination},
    util::{constants::OSU_API_ISSUE, get_combined_thumbnail, MessageExt},
    BotResult, Context,
};

use futures::future::{try_join_all, TryFutureExt};
use itertools::Itertools;
use rayon::prelude::*;
use rosu::models::{GameMode, User};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};
use twilight::model::channel::Message;

#[command]
#[short_desc("Compare the 100 most played maps of multiple users")]
#[long_desc(
    "Compare all users' 100 most played maps and check which \
     ones appear for each user (up to 10 users)"
)]
#[usage("[name1] [name2] ...")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonmostplayed", "mpc")]
async fn mostplayedcommon(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mut args = MultNameArgs::new(&ctx, args, 10);
    let names = match args.names.len() {
        0 => {
            let content = "You need to specify at least one osu username. \
                If you're not linked, you must specify at least two names.";
            return msg.error(&ctx, content).await;
        }
        1 => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
                args.names.push(name);
                args.names
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
        _ => args.names,
    };

    if names.iter().unique().count() == 1 {
        let content = "Give at least two different names";
        return msg.error(&ctx, content).await;
    }

    // Retrieve all users
    let user_futs = names.iter().enumerate().map(|(i, name)| {
        ctx.osu_user(&name, GameMode::STD)
            .map_ok(move |user| (i, user))
    });
    let users: HashMap<u32, User> = match try_join_all(user_futs).await {
        Ok(users) => match users.par_iter().find_any(|(_, user)| user.is_none()) {
            Some((idx, _)) => {
                let content = format!("User `{}` was not found", names[*idx]);
                return msg.error(&ctx, content).await;
            }
            None => users
                .into_iter()
                .filter_map(|(_, user)| user.map(|user| (user.user_id, user)))
                .collect(),
        },
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Retrieve all most played maps and store their count for each user
    let map_futs = users.keys().map(|&id| {
        ctx.clients
            .custom
            .get_most_played(id, 100)
            .map_ok(move |maps| (id, maps))
    });
    let mut users_count: HashMap<u32, HashMap<u32, u32>> = HashMap::with_capacity(users.len());
    let all_maps: HashSet<_> = match try_join_all(map_futs).await {
        Ok(all_maps) => all_maps
            .into_iter()
            .map(|(id, maps)| {
                let map_counts = maps
                    .iter()
                    .map(|map| (map.beatmap_id, map.count))
                    .collect::<HashMap<u32, u32>>();
                users_count.insert(id, map_counts);
                maps.into_iter()
            })
            .flatten()
            .collect(),
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why);
        }
    };

    // Consider only maps that appear in each users map list
    let mut maps: Vec<_> = all_maps
        .into_par_iter()
        .filter(|map| {
            users_count
                .par_iter()
                .all(|(_, count_map)| count_map.contains_key(&map.beatmap_id))
        })
        .collect();
    let amount_common = maps.len();

    // Sort maps by sum of counts
    let total_counts: HashMap<u32, u32> = users_count.iter().fold(
        HashMap::with_capacity(maps.len()),
        |mut counts, (_, user_entry)| {
            for (&map_id, count) in user_entry {
                *counts.entry(map_id).or_default() += count;
            }
            counts
        },
    );
    maps.sort_unstable_by(|a, b| {
        total_counts
            .get(&b.beatmap_id)
            .cmp(&total_counts.get(&a.beatmap_id))
    });

    // Accumulate all necessary data
    let len = names.iter().map(|name| name.len() + 4).sum();
    let mut content = String::with_capacity(len);
    let mut iter = names.into_iter();
    let _ = write!(content, "`{}`", iter.next().unwrap());
    for name in iter {
        let _ = write!(content, ", `{}`", name);
    }
    if amount_common == 0 {
        content.push_str(" don't share any maps in their 100 most played maps");
    } else {
        let _ = write!(
            content,
            " have {}/100 common most played map{}",
            amount_common,
            if amount_common > 1 { "s" } else { "" }
        );
    }

    // Keys have no strict order, hence inconsistent result
    let thumbnail_fut = async {
        let user_ids: Vec<u32> = users.keys().copied().collect();
        get_combined_thumbnail(&ctx, &user_ids).await
    };
    let data_fut = async {
        let initial_maps = &maps[..10.min(maps.len())];
        MostPlayedCommonEmbed::new(&users, initial_maps, &users_count, 0)
    };
    let (thumbnail_result, data) = tokio::join!(thumbnail_fut, data_fut);
    let thumbnail = match thumbnail_result {
        Ok(thumbnail) => Some(thumbnail),
        Err(why) => {
            warn!("Error while combining avatars: {}", why);
            None
        }
    };

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

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = MostPlayedCommonPagination::new(response, users, users_count, maps);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}
