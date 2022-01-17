use std::{collections::BTreeMap, fmt::Write, sync::Arc};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::id::UserId;

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user, UserArgs},
        DoubleResultCow,
    },
    custom_client::{SnipeScoreOrder, SnipeScoreParams},
    database::OsuData,
    embeds::{EmbedData, PlayerSnipeListEmbed},
    pagination::{Pagination, PlayerSnipeListPagination},
    util::{
        constants::{
            common_literals::{ACC, ACCURACY, MISSES, REVERSE, SORT},
            GENERAL_ISSUE, HUISMETBENEN_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::ModSelection,
        CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

#[command]
#[bucket("snipe")]
#[short_desc("List all national #1 scores of a player")]
#[long_desc(
    "List all national #1 scores of a player.\n\
    To specify an order, you must provide `sort=...` with any of these values:\n\
    - `acc`: Sort by accuracy\n \
    - `stars`: Sort by the map's stars\n \
    - `misses`: Sort by amount of misses\n \
    - `length`: Sort by the map's length\n \
    - `scoredate`: Sort by the date when the score was set\n \
    - `mapdate`: Sort by the map's ranked/loved date\n \
    By default the scores will be sorted by pp.\n\
    To reverse the resulting list you can specify `reverse=true`\n\
    Mods can also be specified.\n\
    All data originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    website [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username] [+mods] [sort=acc/stars/misses/length/scoredate/mapdate] [reverse=true/false]")]
#[example("badewanne3 +dt sort=acc reverse=true", "+hdhr sort=scoredate")]
#[aliases("psl")]
async fn playersnipelist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match PlayerListArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(list_args)) => {
                    _playersnipelist(ctx, CommandData::Message { msg, args, num }, list_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_snipe(ctx, *command).await,
    }
}

pub(super) async fn _playersnipelist(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: PlayerListArgs,
) -> BotResult<()> {
    let PlayerListArgs {
        osu,
        order,
        mods,
        descending,
    } = args;

    let name = match osu.map(OsuData::into_username) {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::STD);

    let mut user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = GameMode::STD;

    let country = if ctx.contains_country(user.country_code.as_str()) {
        user.country_code.to_owned()
    } else {
        let content = format!(
            "`{}`'s country {} is not supported :(",
            user.username, user.country_code
        );

        return data.error(&ctx, content).await;
    };

    let params = SnipeScoreParams::new(user.user_id, country)
        .order(order)
        .descending(descending)
        .mods(mods);

    let scores_fut = ctx.clients.custom.get_national_firsts(&params);
    let count_fut = ctx.clients.custom.get_national_firsts_count(&params);

    let (scores, count) = match tokio::try_join!(scores_fut, count_fut) {
        Ok((scores, mut count)) => {
            let scores = scores.into_iter().enumerate().collect::<BTreeMap<_, _>>();

            // TODO: Remove this when it's fixed on huismetbenen
            if params.order != SnipeScoreOrder::Pp {
                count = count.min(1000);
            }

            (scores, count)
        }
        Err(why) => {
            let _ = data.error(&ctx, HUISMETBENEN_ISSUE).await;

            return Err(why.into());
        }
    };

    // Get the first five maps from the database
    let map_ids: Vec<_> = scores
        .values()
        .take(5)
        .map(|score| score.beatmap_id as i32)
        .collect();

    let mut maps = match ctx.psql().get_beatmaps(&map_ids, true).await {
        Ok(maps) => maps,
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to get maps from DB");
            warn!("{:?}", report);

            HashMap::default()
        }
    };

    // Retrieving all missing beatmaps
    for map_id in map_ids {
        let map_id = map_id as u32;

        if !maps.contains_key(&map_id) {
            match ctx.osu().beatmap().map_id(map_id).await {
                Ok(map) => {
                    maps.insert(map_id, map);
                }
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        }
    }

    let pages = numbers::div_euclid(5, count);
    let embed_data = PlayerSnipeListEmbed::new(&user, &scores, &maps, count, (1, pages)).await;

    let mut content = format!(
        "`Order: {order:?} {descending}`",
        order = params.order,
        descending = if params.descending { "Desc" } else { "Asc" },
    );

    if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = params.mods {
        let _ = write!(content, " ~ `Mods: {mods}`");
    }

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = PlayerSnipeListPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        scores,
        maps,
        count,
        params,
    );

    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

pub(super) struct PlayerListArgs {
    pub osu: Option<OsuData>,
    pub order: SnipeScoreOrder,
    pub mods: Option<ModSelection>,
    pub descending: bool,
}

impl PlayerListArgs {
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
        let mut osu = ctx.psql().get_user_osu(author_id).await?;
        let mut order = None;
        let mut mods = None;
        let mut descending = None;

        for arg in args.take(4).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    SORT => {
                        order = match value {
                            ACC | ACCURACY | "a" => Some(SnipeScoreOrder::Accuracy),
                            "mapdate" | "md" => Some(SnipeScoreOrder::MapApprovalDate),
                            MISSES | "miss" | "m" => Some(SnipeScoreOrder::Misses),
                            "scoredate" | "sd" => Some(SnipeScoreOrder::ScoreDate),
                            "stars" | "s" => Some(SnipeScoreOrder::Stars),
                            "length" | "len" | "l" => Some(SnipeScoreOrder::Length),
                            _ => {
                                let content = "Failed to parse `sort`. \
                                Must be either `acc`, `length`, `mapdate`, `misses`, `scoredate`, or `stars`.";

                                return Ok(Err(content.into()));
                            }
                        }
                    }
                    REVERSE => match value {
                        "true" | "t" | "1" => descending = Some(false),
                        "false" | "f" | "0" => descending = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `sort` or `reverse`."
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg.as_ref()) {
                mods = Some(mods_);
            } else {
                match check_user_mention(ctx, arg.as_ref()).await? {
                    Ok(osu_) => osu = Some(osu_),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let args = Self {
            osu,
            order: order.unwrap_or_default(),
            mods,
            descending: descending.unwrap_or(true),
        };

        Ok(Ok(args))
    }
}
