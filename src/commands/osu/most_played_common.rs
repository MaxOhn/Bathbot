use crate::{
    arguments::{Args, MultNameArgs},
    embeds::{EmbedData, MostPlayedCommonEmbed},
    pagination::{MostPlayedCommonPagination, Pagination},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use futures::stream::{FuturesOrdered, StreamExt};
use hashbrown::HashMap;
use itertools::Itertools;
use rosu_v2::prelude::OsuError;
use smallvec::SmallVec;
use std::{cmp::Reverse, fmt::Write, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Compare the 100 most played maps of multiple users")]
#[long_desc(
    "Compare all users' 100 most played maps and check which \
     ones appear for each user (up to 3 users)"
)]
#[usage("[name1] [name2] [name3")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonmostplayed", "mpc")]
async fn mostplayedcommon(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mut args = MultNameArgs::new(&ctx, args, 3);

    let names = match args.names.len() {
        0 => {
            let content = "You need to specify at least one osu username. \
                If you're not linked, you must specify at least two names.";

            return msg.error(&ctx, content).await;
        }
        1 => match ctx.get_link(msg.author.id.0) {
            Some(name) => {
                args.names.insert(0, name);

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

    // Retrieve all most played maps and store their count for each user
    let mut map_futs = names
        .iter()
        .cloned()
        .map(|name| async {
            let fut_1 = ctx.osu().user_most_played(name.as_str()).limit(50);
            let fut_2 = ctx
                .osu()
                .user_most_played(name.as_str())
                .limit(50)
                .offset(50);

            (name, tokio::try_join!(fut_1, fut_2))
        })
        .collect::<FuturesOrdered<_>>();

    let mut users_count = SmallVec::<[HashMap<u32, usize>; 3]>::with_capacity(names.len());
    let mut all_maps = HashMap::with_capacity(names.len() * 80);

    while let Some((name, map_result)) = map_futs.next().await {
        match map_result {
            Ok((mut maps, mut maps_2)) => {
                maps.append(&mut maps_2);

                let map_counts = maps.iter().map(|map| (map.map.map_id, map.count)).collect();
                users_count.push(map_counts);
                all_maps.extend(maps.into_iter().map(|map| (map.map.map_id, map)));
            }
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    drop(map_futs);

    // Consider only maps that appear in each users map list
    let mut maps: Vec<_> = all_maps
        .into_iter()
        .map(|(_, map)| map)
        .filter(|map| {
            users_count
                .iter()
                .all(|count_map| count_map.contains_key(&map.map.map_id))
        })
        .collect();

    let amount_common = maps.len();

    // Sort maps by sum of counts
    let total_counts: HashMap<u32, usize> = users_count.iter().fold(
        HashMap::with_capacity(maps.len()),
        |mut counts, user_entry| {
            for (&map_id, count) in user_entry {
                *counts.entry(map_id).or_default() += count;
            }

            counts
        },
    );

    maps.sort_unstable_by_key(|m| Reverse(total_counts.get(&m.map.map_id)));

    // Accumulate all necessary data
    let len = names.iter().map(|name| name.len() + 4).sum();
    let mut content = String::with_capacity(len);
    let mut iter = names.iter();

    if let Some(first) = iter.next() {
        let last = iter.next_back();
        let _ = write!(content, "`{}`", first);

        for name in iter {
            let _ = write!(content, ", `{}`", name);
        }

        if let Some(name) = last {
            if len > 2 {
                content.push(',');
            }

            let _ = write!(content, " and `{}`", name);
        }
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

    let data_fut = async {
        let initial_maps = &maps[..10.min(maps.len())];

        MostPlayedCommonEmbed::new(&names, initial_maps, &users_count, 0)
    };

    // Creating the embed
    let embed = data_fut.await.into_builder().build();

    // Note: No combined pictures since user ids are not available

    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(content)?
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        response.reaction_delete(&ctx, msg.author.id);

        return Ok(());
    }

    // Pagination
    let pagination = MostPlayedCommonPagination::new(response, names, users_count, maps);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mostcommonplayed): {}")
        }
    });

    Ok(())
}
