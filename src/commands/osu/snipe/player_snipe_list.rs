use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::Write,
    sync::Arc,
};

use command_macros::command;
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};

use crate::{
    commands::osu::{require_link, HasMods, ModsResult},
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::SnipeScoreParams,
    manager::redis::{osu::UserArgs, RedisData},
    pagination::PlayerSnipeListPagination,
    util::{
        constants::{GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE},
        matcher,
        osu::ModSelection,
        ChannelExt, CowUtils,
    },
    Context,
};

use super::{SnipePlayerList, SnipePlayerListOrder};

#[command]
#[desc("List all national #1 scores of a player")]
#[help(
    "List all national #1 scores of a player.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
    - `acc`: Sort by accuracy\n \
    - `stars`: Sort by the map's stars\n \
    - `misses`: Sort by amount of misses\n \
    - `scoredate`: Sort by the date when the score was set\n \
    By default the scores will be sorted by pp.\n\
    To reverse the resulting list you can specify `reverse=true`\n\
    Mods can also be specified.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username] [+mods] [sort=acc/stars/misses/scoredate] [reverse=true/false]")]
#[examples("badewanne3 +dt sort=acc reverse=true", "+hdhr sort=scoredate")]
#[alias("psl")]
#[group(Osu)]
async fn prefix_playersnipelist(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipePlayerList::args(args) {
        Ok(args) => player_list(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub(super) async fn player_list(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: SnipePlayerList<'_>,
) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(ModSelection::Exclude(_)) => {
            let content = "The huismetbenen api unfortunately does not support excluded mods :(";

            return orig.error(&ctx, content).await;
        }
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content =
                "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";

            return orig.error(&ctx, content).await;
        }
    };

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let (country_code, username, user_id) = match &user {
        RedisData::Original(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
        RedisData::Archived(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id)
        }
    };

    let country = if ctx.huismetbenen().is_supported(country_code).await {
        country_code.to_owned()
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(&ctx, content).await;
    };

    let params = SnipeScoreParams::new(user_id, country)
        .order(args.sort.unwrap_or_default())
        .descending(args.reverse.map_or(true, |b| !b))
        .mods(mods);

    let scores_fut = ctx.client().get_national_firsts(&params);
    let count_fut = ctx.client().get_national_firsts_count(&params);

    let (scores, count) = match tokio::try_join!(scores_fut, count_fut) {
        Ok((scores, count)) => {
            let scores: BTreeMap<_, _> = scores.into_iter().enumerate().collect();

            (scores, count)
        }
        Err(err) => {
            let _ = orig.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(err.wrap_err("failed to get scores or counts"));
        }
    };

    // Get the first five maps from the database
    let map_ids = scores
        .values()
        .take(5)
        .map(|score| (score.map.map_id as i32, None))
        .collect();

    let maps = match ctx.osu_map().maps(&map_ids).await {
        Ok(maps) => maps,
        Err(err) => {
            warn!(
                "{:?}",
                Report::new(err).wrap_err("failed to get maps from database")
            );

            HashMap::default()
        }
    };

    let mut content = format!(
        "`Order: {order:?} {descending}`",
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = params.mods {
        let _ = write!(content, " ~ `Mods: {mods}`");
    }

    PlayerSnipeListPagination::builder(user, scores, maps, count, params)
        .content(content)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

impl<'m> SnipePlayerList<'m> {
    fn args(args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut sort = None;
        let mut mods = None;
        let mut reverse = None;

        for arg in args.take(4).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "sort" | "s" => {
                        sort = match value {
                            "acc" | "accuracy" | "a" => Some(SnipePlayerListOrder::Acc),
                            "misses" | "miss" | "m" => Some(SnipePlayerListOrder::Misses),
                            "scoredate" | "sd" => Some(SnipePlayerListOrder::Date),
                            "stars" | "s" => Some(SnipePlayerListOrder::Stars),
                            _ => {
                                let content = "Failed to parse `sort`. \
                                Must be either `acc`, `misses`, `scoredate`, or `stars`.";

                                return Err(content.into());
                            }
                        }
                    }
                    "reverse" | "r" => match value {
                        "true" | "t" | "1" => reverse = Some(true),
                        "false" | "f" | "0" => reverse = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `sort` or `reverse`."
                        );

                        return Err(content.into());
                    }
                }
            } else if matcher::get_mods(&arg).is_some() {
                mods = Some(arg);
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        Ok(Self {
            name,
            mods,
            sort,
            reverse,
            discord,
        })
    }
}
