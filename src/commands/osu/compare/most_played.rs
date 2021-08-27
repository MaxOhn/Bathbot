use super::TripleArgs;
use crate::{
    embeds::{EmbedData, MostPlayedCommonEmbed},
    pagination::{MostPlayedCommonPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use futures::stream::{FuturesOrdered, StreamExt};
use hashbrown::{HashMap, HashSet};
use rosu_v2::prelude::OsuError;
use smallvec::SmallVec;
use std::{cmp::Reverse, fmt::Write, sync::Arc};

#[command]
#[short_desc("Compare the 100 most played maps of multiple users")]
#[long_desc(
    "Compare all users' 100 most played maps and check which \
     ones appear for each user (up to 3 users)"
)]
#[usage("[name1] [name2] [name3")]
#[example("badewanne3 \"nathan on osu\" idke")]
#[aliases("commonmostplayed", "mpc")]
async fn mostplayedcommon(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match TripleArgs::args(&ctx, &mut args, msg.author.id, None).await {
                Ok(Ok(mostplayed_args)) => {
                    let data = CommandData::Message { msg, args, num };

                    _mostplayedcommon(ctx, data, mostplayed_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_compare(ctx, command).await,
    }
}

pub(super) async fn _mostplayedcommon(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: TripleArgs,
) -> BotResult<()> {
    let TripleArgs {
        name1,
        name2,
        name3,
        mode: _,
    } = args;

    let author_id = data.author()?.id;

    let name1 = match name1 {
        Some(name) => name,
        None => {
            let content =
                "Since you're not linked with the `link` command, you must specify two names.";

            return data.error(&ctx, content).await;
        }
    };

    let mut names = Vec::with_capacity(3);
    names.push(name1);
    names.push(name2);

    if let Some(name) = name3 {
        names.push(name);
    }

    {
        let unique: HashSet<_> = names.iter().collect();

        if unique.len() == 1 {
            let content = "Give at least two different names";

            return data.error(&ctx, content).await;
        } else if unique.len() < names.len() {
            drop(unique);

            names.dedup(); // * Note: Doesn't consider [a, b, a] but whatever
        }
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

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

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
    let mut content = String::with_capacity(16);
    let len = names.len();
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
        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        return Ok(());
    }

    let _ = write!(
        content,
        " have {}/100 common most played map{}",
        amount_common,
        if amount_common > 1 { "s" } else { "" }
    );

    let data_fut = async {
        let initial_maps = &maps[..10.min(maps.len())];

        MostPlayedCommonEmbed::new(&names, initial_maps, &users_count, 0)
    };

    // Creating the embed
    let embed = data_fut.await.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);

    // * Note: No combined pictures since user ids are not available

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MostPlayedCommonPagination::new(response, names, users_count, maps);
    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mostplayedcommon): {}")
        }
    });

    Ok(())
}
