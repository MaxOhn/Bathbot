use crate::{
    arguments::MultNameArgs,
    embeds::BasicEmbedData,
    scraper::MostPlayedMap,
    util::{discord, globals::OSU_API_ISSUE},
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
    iter::Extend,
};
use tokio::runtime::Runtime;

#[command]
#[description = "Compare all users' 100 most played maps and check which \
                 ones appear for each user (up to 10 users)"]
#[usage = "[name1] [name2] ..."]
#[example = "badewanne3 \"nathan on osu\" idke"]
#[aliases("commonmostplayed", "mpc")]
fn mostplayedcommon(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let mut args = MultNameArgs::new(args, 10);
    let names = match args.names.len() {
        0 => {
            msg.channel_id.say(
                &ctx.http,
                "You need to specify at least one osu username. \
                 If you're not linked, you must specify at least two names.",
            )?;
            return Ok(());
        }
        1 => {
            let data = ctx.data.read();
            let links = data
                .get::<DiscordLinks>()
                .expect("Could not get DiscordLinks");
            match links.get(msg.author.id.as_u64()) {
                Some(name) => {
                    args.names.insert(name.clone());
                }
                None => {
                    msg.channel_id.say(
                        &ctx.http,
                        "Since you're not linked via `<link`, \
                         you must specify at least two names.",
                    )?;
                    return Ok(());
                }
            }
            args.names
        }
        _ => args.names,
    };
    if names.iter().collect::<HashSet<_>>().len() == 1 {
        msg.channel_id
            .say(&ctx.http, "Give at least two different names.")?;
        return Ok(());
    }
    let mut rt = Runtime::new().unwrap();

    // Retrieve users and their most played maps
    let mut users: HashMap<u32, User> = HashMap::with_capacity(names.len());
    let mut users_count: HashMap<u32, HashMap<u32, u32>> = HashMap::with_capacity(names.len());
    let mut all_maps: HashSet<MostPlayedMap> = HashSet::with_capacity(names.len() * 99);
    {
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get Osu");
        let scraper = data.get::<Scraper>().expect("Could not get Scraper");
        for name in names.iter() {
            let req = UserRequest::with_username(&name);
            let user = match rt.block_on(req.queue_single(&osu)) {
                Ok(result) => match result {
                    Some(user) => user,
                    None => {
                        msg.channel_id
                            .say(&ctx.http, format!("User `{}` was not found", name))?;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
                    return Err(CommandError::from(why.to_string()));
                }
            };
            let maps = {
                match rt.block_on(scraper.get_most_played(user.user_id, 100)) {
                    Ok(maps) => maps,
                    Err(why) => {
                        msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
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
    let all_maps: Vec<_> = all_maps
        .into_iter()
        .filter(|map| {
            users_count
                .iter()
                .all(|(_, count_map)| count_map.contains_key(&map.beatmap_id))
        })
        .collect();
    let amount_common = all_maps.len();

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
        content.push_str(&format!(
            " have {}/100 common most played map{}",
            amount_common,
            if amount_common > 1 { "s" } else { "" }
        ));
        if amount_common > 10 {
            content.push_str(", here's the top 10 of them:");
        } else {
            content.push(':');
        }
    }
    let (data, thumbnail) = BasicEmbedData::create_mostplayedcommon(users, all_maps, users_count);

    // Creating the embed
    let response = msg.channel_id.send_message(&ctx.http, |m| {
        if !thumbnail.is_empty() {
            let bytes: &[u8] = &thumbnail;
            m.add_file((bytes, "avatar_fuse.png"));
        }
        m.content(content)
            .embed(|e| data.build(e).thumbnail("attachment://avatar_fuse.png"))
    });

    // Save the response owner
    discord::save_response_owner(response?.id, msg.author.id, ctx.data.clone());
    Ok(())
}
