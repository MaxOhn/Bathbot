use crate::{
    database::OsuData,
    embeds::{EmbedData, SnipedDiffEmbed},
    pagination::{Pagination, SnipedDiffPagination},
    util::{
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use chrono::{Duration, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};
use std::{cmp::Reverse, sync::Arc};

pub(super) async fn _sniped_diff(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    diff: Difference,
    name: Option<Name>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Request the user
    let mut user = match super::request_user(&ctx, &name, GameMode::STD).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("Could not find user `{}`", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = GameMode::STD;

    if !ctx.contains_country(user.country_code.as_str()) {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return data.error(&ctx, content).await;
    }

    let client = &ctx.clients.custom;
    let now = Utc::now();
    let week_ago = now - Duration::weeks(1);

    // Request the scores
    let scores_fut = match diff {
        Difference::Gain => client.get_national_snipes(&user, true, week_ago, now),
        Difference::Loss => client.get_national_snipes(&user, false, week_ago, now),
    };

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(why.into());
        }
    };

    if scores.is_empty() {
        let content = format!(
            "`{name}` didn't {diff} national #1s in the last week.",
            name = user.username,
            diff = match diff {
                Difference::Gain => "gain any new",
                Difference::Loss => "lose any",
            }
        );

        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        return Ok(());
    }

    scores.sort_unstable_by_key(|s| Reverse(s.date));

    let pages = numbers::div_euclid(5, scores.len());
    let mut maps = HashMap::new();

    let data_fut = SnipedDiffEmbed::new(&user, diff, &scores, 0, (1, pages), &mut maps);

    let builder = match data_fut.await {
        Ok(data) => data.into_builder().build().into(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = SnipedDiffPagination::new(response, user, diff, scores, maps);

    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's recently acquired national #1 scores")]
#[long_desc(
    "Display a user's national #1 scores that they acquired within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("sg", "snipegain", "snipesgain")]
#[bucket("snipe")]
async fn snipedgain(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            let data = CommandData::Message { msg, args, num };

            _sniped_diff(ctx, data, Difference::Gain, name).await
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's recently lost national #1 scores")]
#[long_desc(
    "Display a user's national #1 scores that they lost within the last week.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "sl",
    "snipeloss",
    "snipesloss",
    "snipedlost",
    "snipelost",
    "snipeslost"
)]
#[bucket("snipe")]
async fn snipedloss(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            let data = CommandData::Message { msg, args, num };

            _sniped_diff(ctx, data, Difference::Loss, name).await
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

#[derive(Copy, Clone)]
pub enum Difference {
    Gain,
    Loss,
}
