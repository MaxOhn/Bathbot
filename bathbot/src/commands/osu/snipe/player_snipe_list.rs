use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::Write,
};

use bathbot_macros::command;
use bathbot_model::SnipeScoreParams;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::ModSelection,
    CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};

use super::{SnipeGameMode, SnipePlayerList, SnipePlayerListOrder};
use crate::{
    active::{impls::SnipePlayerListPagination, ActiveMessages},
    commands::osu::{HasMods, ModsResult},
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::ChannelExt,
    Context,
};

#[command]
#[desc("List all national #1 scores of a player")]
#[help(
    "List all national #1 scores of a player.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
    - `acc`: Sort by accuracy\n\
    - `stars`: Sort by the map's stars\n\
    - `misses`: Sort by amount of misses\n\
    - `scoredate`: Sort by the date when the score was set\n\
    By default the scores will be sorted by pp.\n\
    To reverse the resulting list you can specify `reverse=true`\n\
    Mods can also be specified.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username] [+mods] [sort=acc/stars/misses/scoredate] [reverse=true/false]")]
#[examples("badewanne3 +dt sort=acc reverse=true", "+hdhr sort=scoredate")]
#[alias("psl")]
#[group(Osu)]
async fn prefix_playersnipelist(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipePlayerList::args(args, GameMode::Osu) {
        Ok(args) => player_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("List all national #1 ctb scores of a player")]
#[help(
    "List all national #1 ctb scores of a player.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
    - `acc`: Sort by accuracy\n\
    - `stars`: Sort by the map's stars\n\
    - `misses`: Sort by amount of misses\n\
    - `scoredate`: Sort by the date when the score was set\n\
    By default the scores will be sorted by pp.\n\
    To reverse the resulting list you can specify `reverse=true`\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username] [sort=acc/stars/misses/scoredate] [reverse=true/false]")]
#[examples("badewanne3 sort=acc reverse=true", "sort=scoredate")]
#[alias("pslc", "playersnipelistcatch")]
#[group(Catch)]
async fn prefix_playersnipelistctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipePlayerList::args(args, GameMode::Catch) {
        Ok(args) => player_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("List all national #1 mania scores of a player")]
#[help(
    "List all national #1 mania scores of a player.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
    - `acc`: Sort by accuracy\n\
    - `stars`: Sort by the map's stars\n\
    - `misses`: Sort by amount of misses\n\
    - `scoredate`: Sort by the date when the score was set\n\
    By default the scores will be sorted by pp.\n\
    To reverse the resulting list you can specify `reverse=true`\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username] [sort=acc/stars/misses/scoredate] [reverse=true/false]")]
#[examples("badewanne3 sort=acc reverse=true", "sort=scoredate")]
#[alias("pslm")]
#[group(Mania)]
async fn prefix_playersnipelistmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match SnipePlayerList::args(args, GameMode::Mania) {
        Ok(args) => player_list(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

pub(super) async fn player_list(orig: CommandOrigin<'_>, args: SnipePlayerList<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(ModSelection::Exclude(_)) => {
            let content = "Excluded mods unfortunately are not supported :(";

            return orig.error(content).await;
        }
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content =
                "Failed to parse mods. Be sure to specify a valid abbreviation e.g. `hdhr`.";

            return orig.error(content).await;
        }
    };

    let owner = orig.user_id()?;

    let (user_id, mode) = user_id_mode!(orig, args);
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
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
        RedisData::Archive(user) => {
            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id;

            (country_code, username, user_id.to_native())
        }
    };

    let country = if Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        country_code.to_owned()
    } else {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(content).await;
    };

    let params = SnipeScoreParams::new(user_id, &country, mode)
        .order(args.sort.unwrap_or_default())
        .descending(args.reverse.map_or(true, |b| !b))
        .mods(mods);

    let client = Context::client();
    let scores_fut = client.get_national_firsts(&params);
    let count_fut = client.get_national_firsts_count(&params);

    let (scores, count) = match tokio::try_join!(scores_fut, count_fut) {
        Ok((scores, count)) => {
            let scores: BTreeMap<_, _> = scores.into_iter().enumerate().collect();

            (scores, count)
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get scores or counts"));
        }
    };

    // Get the first five maps from the database
    let map_ids = scores
        .values()
        .take(5)
        .map(|score| (score.map_id as i32, None))
        .collect();

    let maps = match Context::osu_map().maps(&map_ids).await {
        Ok(maps) => maps,
        Err(err) => {
            warn!(?err, "Failed to get maps from database");

            HashMap::default()
        }
    };

    let mut content = format!(
        "`Order: {order:?} {descending}`",
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(ModSelection::Exact(ref mods)) | Some(ModSelection::Include(ref mods)) = params.mods
    {
        let _ = write!(content, " ~ `Mods: {mods}`");
    }

    let pagination = SnipePlayerListPagination::builder()
        .user(user)
        .scores(scores)
        .maps(maps)
        .total(count)
        .params(params)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

impl<'m> SnipePlayerList<'m> {
    fn args(args: Args<'m>, mode: GameMode) -> Result<Self, Cow<'static, str>> {
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
            mode: SnipeGameMode::try_from_mode(mode),
            name,
            mods,
            sort,
            reverse,
            discord,
        })
    }
}
