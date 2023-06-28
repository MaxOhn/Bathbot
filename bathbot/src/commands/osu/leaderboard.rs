use std::{borrow::Cow, collections::HashMap, sync::Arc};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{AVATAR_URL, GENERAL_ISSUE, OSU_WEB_ISSUE},
    matcher,
    osu::{MapIdType, ModSelection},
};
use eyre::{Report, Result};
use rosu_v2::prelude::{
    BeatmapUserScore, GameMode, GameMods, GameModsIntermode, Grade, OsuError, ScoreStatistics,
    Username,
};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Message},
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::{HasMods, ModsResult};
use crate::{
    active::{impls::LeaderboardPagination, ActiveMessages},
    core::commands::{prefix::Args, CommandOrigin},
    manager::{
        redis::{osu::UserArgs, RedisData},
        MapError,
    },
    util::{interaction::InteractionCommand, ChannelExt, CheckPermissions, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "leaderboard", desc = "Display the global leaderboard of a map")]
pub struct Leaderboard<'a> {
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
    )]
    mods: Option<Cow<'a, str>>,
}

#[derive(HasMods)]
struct LeaderboardArgs<'a> {
    map: Option<MapIdType>,
    mods: Option<Cow<'a, str>>,
}

impl<'m> LeaderboardArgs<'m> {
    async fn args(
        ctx: &Context,
        msg: &Message,
        args: Args<'m>,
    ) -> Result<LeaderboardArgs<'m>, String> {
        let mut map = None;
        let mut mods = None;

        for arg in args.take(2) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                map = Some(id);
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    Must be either a map id, map url, or mods.",
                );

                return Err(content);
            }
        }

        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(reply) = reply {
            if let Some(id) = ctx.find_map_id_in_msg(reply).await {
                map = Some(id);
            }
        }

        Ok(Self { map, mods })
    }
}

impl<'a> TryFrom<Leaderboard<'a>> for LeaderboardArgs<'a> {
    type Error = &'static str;

    fn try_from(args: Leaderboard<'a>) -> Result<Self, Self::Error> {
        let map = match args.map {
            Some(map) => {
                if let Some(id) = matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
                {
                    Some(id)
                } else {
                    return Err(
                        "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
                    );
                }
            }
            None => None,
        };

        Ok(Self {
            map,
            mods: args.mods,
        })
    }
}

#[command]
#[desc("Display the global leaderboard of a map")]
#[help(
    "Display the global leaderboard of a given map.\n\
    If no map is given, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[alias("lb")]
#[group(AllModes)]
async fn prefix_leaderboard(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match LeaderboardArgs::args(&ctx, msg, args).await {
        Ok(args) => leaderboard(ctx, CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_leaderboard(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Leaderboard::from_interaction(command.input_data())?;

    match LeaderboardArgs::try_from(args) {
        Ok(args) => leaderboard(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn leaderboard(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: LeaderboardArgs<'_>,
) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let owner = orig.user_id()?;

    let map_id_fut = get_map_id(&ctx, &orig, args.map);
    let osu_id_fut = ctx.user_config().osu_id(owner);

    let (map_id_res, osu_id_res) = tokio::join!(map_id_fut, osu_id_fut);

    let map_id = match map_id_res {
        Ok(map_id) => map_id,
        Err(GetMapError::Content(content)) => return orig.error(&ctx, content).await,
        Err(GetMapError::Err { err, content }) => {
            let _ = orig.error(&ctx, content).await;

            return Err(err);
        }
    };

    // Retrieving the beatmap
    let map = match ctx.osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?",
            );

            return orig.error(&ctx, content).await;
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mods = match mods {
        Some(ModSelection::Include(mods) | ModSelection::Exact(mods)) => Some(mods),
        Some(ModSelection::Exclude(_)) | None => None,
    };

    let mods_bits = mods.as_ref().map_or(0, GameModsIntermode::bits);

    let mut calc = ctx.pp(&map).mode(map.mode()).mods(mods_bits);
    let attrs_fut = calc.performance();

    let scores_fut = ctx
        .osu_scores()
        .map_leaderboard(map_id, map.mode(), mods.clone());

    let user_fut = get_user_score(&ctx, osu_id_res, map_id, map.mode(), mods.clone());

    let (scores_res, user_res, attrs) = tokio::join!(scores_fut, user_fut, attrs_fut);

    let scores = match scores_res {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(err.wrap_err("Failed to get leaderboard"));
        }
    };

    let user_score = user_res
        .unwrap_or_else(|err| {
            warn!(?err, "Failed to get user score");

            None
        })
        .map(|(user, score)| LeaderboardUserScore {
            discord_id: owner,
            user_id: user.user_id(),
            username: user.username().into(),
            pos: score.pos,
            grade: score.score.grade,
            accuracy: score.score.accuracy,
            statistics: score.score.statistics,
            mods: score.score.mods,
            pp: score.score.pp,
            combo: score.score.max_combo,
            score: score.score.score,
            ended_at: score.score.ended_at,
        });

    let amount = scores.len();

    let content = if mods.is_some() {
        format!("I found {amount} scores with the specified mods on the map's leaderboard")
    } else {
        format!("I found {amount} scores on the map's leaderboard")
    };

    // Accumulate all necessary data
    let first_place_icon = scores.first().map(|s| format!("{AVATAR_URL}{}", s.user_id));

    let mut attr_map = HashMap::default();
    let stars = attrs.stars() as f32;
    let max_pp = attrs.pp() as f32;
    let max_combo = attrs.max_combo() as u32;
    attr_map.insert(mods_bits, (attrs.into(), max_pp));

    let pagination = LeaderboardPagination::builder()
        .map(map)
        .scores(scores.into_boxed_slice())
        .stars(stars)
        .max_combo(max_combo)
        .attr_map(attr_map)
        .author_data(user_score)
        .first_place_icon(first_place_icon)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

enum GetMapError {
    Content(&'static str),
    Err { err: Report, content: &'static str },
}

async fn get_map_id(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    map: Option<MapIdType>,
) -> Result<u32, GetMapError> {
    match map {
        Some(MapIdType::Map(id)) => Ok(id),
        Some(MapIdType::Set(_)) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            Err(GetMapError::Content(content))
        }
        None if orig.can_read_history() => {
            let msgs = ctx
                .retrieve_channel_history(orig.channel_id())
                .await
                .map_err(|err| GetMapError::Err {
                    err,
                    content: GENERAL_ISSUE,
                })?;

            match ctx.find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => Ok(id),
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                        Try specifying a map either by url to the map, or just by map id.";

                    Err(GetMapError::Content(content))
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            Err(GetMapError::Content(content))
        }
    }
}

async fn get_user_score(
    ctx: &Context,
    osu_id_res: Result<Option<u32>>,
    map_id: u32,
    mode: GameMode,
    mods: Option<GameModsIntermode>,
) -> Result<Option<(RedisData<User>, BeatmapUserScore)>> {
    let osu_id = match osu_id_res {
        Ok(osu_id) => osu_id,
        Err(err) => {
            warn!(?err, "Failed to get user config");

            return Ok(None);
        }
    };

    let Some(user_id) = osu_id else {
        return Ok(None);
    };

    let user_args = UserArgs::user_id(user_id).mode(mode);
    let user_fut = ctx.redis().osu_user(user_args);

    let mut score_fut = ctx.osu().beatmap_user_score(map_id, user_id).mode(mode);

    if let Some(mods) = mods {
        score_fut = score_fut.mods(mods);
    }

    match tokio::try_join!(user_fut, score_fut) {
        Ok(tuple) => Ok(Some(tuple)),
        Err(OsuError::NotFound) => Ok(None),
        Err(err) => Err(Report::new(err).wrap_err("Failed to get score or user")),
    }
}

pub struct LeaderboardUserScore {
    pub discord_id: Id<UserMarker>,
    pub user_id: u32,
    pub username: Username,
    pub pos: usize,
    pub grade: Grade,
    pub accuracy: f32,
    pub statistics: ScoreStatistics,
    pub mods: GameMods,
    pub pp: Option<f32>,
    pub combo: u32,
    pub score: u32,
    pub ended_at: OffsetDateTime,
}
