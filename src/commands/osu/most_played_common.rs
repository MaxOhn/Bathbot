use crate::{
    arguments::MultNameArgs,
    embeds::BasicEmbedData,
    pagination::{MostPlayedCommonPagination, Pagination},
    scraper::MostPlayedMap,
    util::{discord, globals::OSU_API_ISSUE, MessageExt},
    DiscordLinks, Osu, Scraper,
};

use itertools::Itertools;
use rosu::{backend::requests::UserRequest, models::User};
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::{
    collections::{HashMap, HashSet},
    convert::From,
    fmt::Write,
    iter::Extend,
    sync::Arc,
};

#[command]
#[description = "Compare all users' 100 most played maps and check which \
                 ones appear for each user (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonmostplayed", "mpc")]
async fn mostplayedcommon(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut args = MultNameArgs::new(args, 10);
    let names = match args.names.len() {
        0 => {
            msg.channel_id
                .say(
                    ctx,
                    "You need to specify at least one osu username. \
                    If you're not linked, you must specify at least two names.",
                )
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Ok(());
        }
        1 => {
            let data = ctx.data.read().await;
            let links = data.get::<DiscordLinks>().unwrap();
            match links.get(msg.author.id.as_u64()) {
                Some(name) => {
                    args.names.insert(name.clone());
                }
                None => {
                    msg.channel_id
                        .say(
                            ctx,
                            "Since you're not linked via `<link`, \
                            you must specify at least two names.",
                        )
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Ok(());
                }
            }
            args.names
        }
        _ => args.names,
    };
    if names.iter().collect::<HashSet<_>>().len() == 1 {
        msg.channel_id
            .say(ctx, "Give at least two different names.")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    }

    // Retrieve users and their most played maps
    let mut users: HashMap<u32, User> = HashMap::with_capacity(names.len());
    let mut users_count: HashMap<u32, HashMap<u32, u32>> = HashMap::with_capacity(names.len());
    let mut all_maps: HashSet<MostPlayedMap> = HashSet::with_capacity(names.len() * 99);
    {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let scraper = data.get::<Scraper>().unwrap();
        for name in names.iter() {
            let req = UserRequest::with_username(&name);
            let user = match req.queue_single(&osu).await {
                Ok(result) => match result {
                    Some(user) => user,
                    None => {
                        msg.channel_id
                            .say(ctx, format!("User `{}` was not found", name))
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id
                        .say(ctx, OSU_API_ISSUE)
                        .await?
                        .reaction_delete(ctx, msg.author.id)
                        .await;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            let maps = {
                match scraper.get_most_played(user.user_id, 100).await {
                    Ok(maps) => maps,
                    Err(why) => {
                        msg.channel_id
                            .say(ctx, OSU_API_ISSUE)
                            .await?
                            .reaction_delete(ctx, msg.author.id)
                            .await;
                        return Err(CommandError::from(why.to_string()));
                    }
                }
            };
            users_count.insert(
                user.user_id,
                maps.iter()
                    .map(|map| (map.beatmap_id, map.count))
                    .collect::<HashMap<u32, u32>>(),
            );
            users.insert(user.user_id, user);
            all_maps.extend(maps.into_iter());
        }
    }

    // Consider only maps that appear in each users map list
    let mut maps: Vec<_> = all_maps
        .into_iter()
        .filter(|map| {
            users_count
                .iter()
                .all(|(_, count_map)| count_map.contains_key(&map.beatmap_id))
        })
        .collect();
    let amount_common = maps.len();

    // Sort maps by sum of counts
    let total_counts: HashMap<u32, u32> = users_count.iter().fold(
        HashMap::with_capacity(maps.len()),
        |mut counts, (_, user_entry)| {
            for (map_id, count) in user_entry {
                *counts.entry(*map_id).or_insert(0) += count;
            }
            counts
        },
    );
    maps.sort_by(|a, b| {
        total_counts
            .get(&b.beatmap_id)
            .cmp(&total_counts.get(&a.beatmap_id))
    });

    // Accumulate all necessary data
    let len = names.len();
    let names_join = names
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(len - 1)
        .map(|chunk| chunk.join("`, `"))
        .join("` and `");
    let mut content = format!("`{}`", names_join);
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
    let user_ids: Vec<u32> = users.keys().copied().collect();
    let thumbnail = match discord::get_combined_thumbnail(&user_ids).await {
        Ok(thumbnail) => thumbnail,
        Err(why) => {
            warn!("Error while combining avatars: {}", why);
            Vec::default()
        }
    };

    let data = BasicEmbedData::create_mostplayedcommon(
        &users,
        &maps[..10.min(maps.len())],
        &users_count,
        0,
    )
    .await;

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| {
            if !thumbnail.is_empty() {
                let bytes: &[u8] = &thumbnail;
                m.add_file((bytes, "avatar_fuse.png"));
            }
            m.content(content)
                .embed(|e| data.build(e).thumbnail("attachment://avatar_fuse.png"))
        })
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = MostPlayedCommonPagination::new(
        ctx,
        resp,
        msg.author.id,
        users,
        users_count,
        maps,
        "attachment://avatar_fuse.png".to_owned(),
    )
    .await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}
