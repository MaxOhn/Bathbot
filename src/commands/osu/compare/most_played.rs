use std::{cmp::Reverse, fmt::Write, sync::Arc};

use command_macros::command;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, MostPlayedMap, OsuError},
    OsuResult,
};
use smallvec::SmallVec;

use crate::{
    commands::osu::{NameExtraction, UserArgs},
    core::commands::CommandOrigin,
    embeds::{EmbedData, MostPlayedCommonEmbed},
    pagination::{MostPlayedCommonPagination, Pagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher,
    },
    BotResult, Context,
};

use super::{CompareMostPlayed, AT_LEAST_ONE};

#[command]
#[desc("Compare the 100 most played maps of two users")]
#[help(
    "Compare the users' 100 most played maps and check which \
     ones appear for each user"
)]
#[usage("[name1] [name2]")]
#[example("badewanne3 \"nathan on osu\"")]
#[aliases("commonmostplayed", "mpc")]
#[group(AllModes)]
async fn prefix_mostplayedcommon(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let mut args_ = CompareMostPlayed::default();

    for arg in args.take(2) {
        if let Some(id) = matcher::get_mention_user(arg) {
            if args_.discord1.is_none() {
                args_.discord1 = Some(id);
            } else {
                args_.discord2 = Some(id);
            }
        } else if args_.name1.is_none() {
            args_.name1 = Some(arg.into());
        } else {
            args_.name2 = Some(arg.into());
        }
    }

    mostplayed(ctx, msg.into(), args_).await
}

async fn extract_name(ctx: &Context, args: &mut CompareMostPlayed<'_>) -> NameExtraction {
    if let Some(name) = args.name1.take().or_else(|| args.name2.take()) {
        NameExtraction::Name(name.as_ref().into())
    } else if let Some(discord) = args.discord1.take().or_else(|| args.discord2.take()) {
        match ctx.psql().get_user_osu(discord).await {
            Ok(Some(osu)) => NameExtraction::Name(osu.into_username()),
            Ok(None) => {
                NameExtraction::Content(format!("<@{discord}> is not linked to an osu!profile"))
            }
            Err(err) => NameExtraction::Err(err),
        }
    } else {
        NameExtraction::None
    }
}

pub(super) async fn mostplayed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    mut args: CompareMostPlayed<'_>,
) -> BotResult<()> {
    let owner = orig.user_id()?;

    let name1 = match extract_name(&ctx, &mut args).await {
        NameExtraction::Name(name) => name,
        NameExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        NameExtraction::Content(content) => return orig.error(&ctx, content).await,
        NameExtraction::None => return orig.error(&ctx, AT_LEAST_ONE).await,
    };

    let name2 = match extract_name(&ctx, &mut args).await {
        NameExtraction::Name(name) => name,
        NameExtraction::Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
        NameExtraction::Content(content) => return orig.error(&ctx, content).await,
        NameExtraction::None => match ctx.psql().get_user_osu(owner).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => {
                let content =
                    "Since you're not linked with the `/link` command, you must specify two names.";

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let fut1 = get_scores_(&ctx, &name1);
    let fut2 = get_scores_(&ctx, &name2);

    let (maps1, maps2) = match tokio::join!(fut1, fut2) {
        (Ok(maps1), Ok(maps2)) => (maps1, maps2),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{name1}` was not found");

            return orig.error(&ctx, content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = format!("User `{name2}` was not found");

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // TODO: find nicer structure
    let mut users_count = SmallVec::<[HashMap<u32, usize>; 3]>::with_capacity(2);
    let mut all_maps = HashMap::with_capacity(100);

    let map_counts = maps1
        .iter()
        .map(|map| (map.map.map_id, map.count))
        .collect();
    users_count.push(map_counts);
    all_maps.extend(maps1.into_iter().map(|map| (map.map.map_id, map)));

    let map_counts = maps2
        .iter()
        .map(|map| (map.map.map_id, map.count))
        .collect();
    users_count.push(map_counts);
    all_maps.extend(maps2.into_iter().map(|map| (map.map.map_id, map)));

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
    let mut content = format!("`{name1}` and `{name2}`");

    if amount_common == 0 {
        content.push_str(" don't share any maps in their 100 most played maps");
        let builder = MessageBuilder::new().embed(content);
        orig.create_message(&ctx, &builder).await?;

        return Ok(());
    }

    let _ = write!(
        content,
        " have {amount_common}/100 common most played map{}",
        if amount_common > 1 { "s" } else { "" }
    );

    // TODO: i hate this
    let names = vec![name1, name2];

    let data_fut = async {
        let initial_maps = &maps[..10.min(maps.len())];

        MostPlayedCommonEmbed::new(&names, initial_maps, &users_count, 0)
    };

    // Creating the embed
    let embed = data_fut.await.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);

    // * Note: No combined pictures since user ids are not available

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MostPlayedCommonPagination::new(response, names, users_count, maps);

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn get_scores_(ctx: &Context, name: &str) -> OsuResult<Vec<MostPlayedMap>> {
    let user_args = UserArgs::new(name, GameMode::STD);
    let scores_fut = ctx.osu().user_most_played(name).limit(100);

    if let Some(alt_name) = user_args.whitespaced_name() {
        match scores_fut.await {
            Ok(maps) => Ok(maps),
            Err(OsuError::NotFound) => {
                ctx.osu()
                    .user_most_played(alt_name.as_str())
                    .limit(100)
                    .await
            }
            Err(err) => Err(err),
        }
    } else {
        scores_fut.await
    }
}
