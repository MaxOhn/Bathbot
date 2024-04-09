use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasMods, HasName, SlashCommand};
use bathbot_model::{rosu_v2::user::User, ScoreSlim};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::{MapIdType, ModSelection},
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMod, GameMode, GameModsIntermode, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    channel::{message::MessageType, Message},
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::{require_link, user_not_found, HasMods, ModsResult};
use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, FixScoreEmbed},
    manager::{
        redis::{
            osu::{UserArgs, UserArgsSlim},
            RedisData,
        },
        MapError, OsuMap,
    },
    util::{interaction::InteractionCommand, osu::IfFc, CheckPermissions, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "fix",
    desc = "Display a user's pp after unchoking their score on a map"
)]
pub struct Fix<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.\
        Alternatively, you can also provide a score url."
    )]
    map: Option<String>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(HasMods, HasName)]
struct FixArgs<'a> {
    name: Option<Cow<'a, str>>,
    id: Option<MapOrScore>,
    mods: Option<Cow<'a, str>>,
    discord: Option<Id<UserMarker>>,
}

enum MapOrScore {
    Map(MapIdType),
    Score { id: u64, mode: GameMode },
}

impl<'m> FixArgs<'m> {
    async fn args(ctx: &Context, msg: &Message, args: Args<'m>) -> FixArgs<'m> {
        let mut name = None;
        let mut discord = None;
        let mut id_ = None;
        let mut mods = None;

        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(reply) = reply {
            if let Some(id) = ctx.find_map_id_in_msg(reply).await {
                id_ = Some(MapOrScore::Map(id));
            } else if let Some((mode, id)) = matcher::get_osu_score_id(&reply.content) {
                id_ = Some(MapOrScore::Score { mode, id });
            }
        }

        for arg in args.take(3) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                id_ = Some(MapOrScore::Map(id));
            } else if let Some((mode, id)) = matcher::get_osu_score_id(arg) {
                id_ = Some(MapOrScore::Score { mode, id });
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            name,
            discord,
            id: id_,
            mods,
        }
    }
}

impl<'a> TryFrom<Fix<'a>> for FixArgs<'a> {
    type Error = &'static str;

    fn try_from(args: Fix<'a>) -> Result<Self, Self::Error> {
        let id = match args.map {
            Some(map) => {
                if let Some(id) = matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
                {
                    Some(MapOrScore::Map(id))
                } else if let Some((mode, id)) = matcher::get_osu_score_id(&map) {
                    Some(MapOrScore::Score { mode, id })
                } else {
                    return Err(
                        "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
                    );
                }
            }
            None => None,
        };

        Ok(Self {
            name: args.name,
            id,
            mods: args.mods,
            discord: args.discord,
        })
    }
}

async fn slash_fix(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Fix::from_interaction(command.input_data())?;

    match FixArgs::try_from(args) {
        Ok(args) => fix(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's pp after unchoking their score on a map")]
#[help(
    "Display a user's pp after unchoking their score on a map. \n\
     If no map is given, I will choose the last map \
     I can find in the embeds of this channel.\n\
     Mods can be specified but only if there already is a score \
     on the map with those mods."
)]
#[alias("fixscore")]
#[usage("[username] [map url / map id] [+mods]")]
#[examples(
    "badewanne3",
    "badewanne3 2240404 +hdhr",
    "https://osu.ppy.sh/beatmapsets/902425#osu/2240404"
)]
#[group(AllModes)]
async fn prefix_fix(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    let args = FixArgs::args(&ctx, msg, args).await;

    fix(ctx, CommandOrigin::from_msg(msg, permissions), args).await
}

async fn fix(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: FixArgs<'_>) -> Result<()> {
    let owner = orig.user_id()?;
    let config = ctx.user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods. Be sure to either specify them directly \
            or through the `+mods` / `+mods!` syntax e.g. `hdhr` or `+hdhr!`";

            return orig.error(&ctx, content).await;
        }
    };

    let legacy_scores = match config.legacy_scores {
        Some(legacy_scores) => legacy_scores,
        None => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.legacy_scores)
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let mods = match mods {
        None | Some(ModSelection::Exclude(_)) => None,
        Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) => Some(mods),
    };

    let data_result = match args.id {
        Some(MapOrScore::Score { id, mode }) => {
            request_by_score(&ctx, &orig, id, mode, user_id, legacy_scores).await
        }
        Some(MapOrScore::Map(MapIdType::Map(id))) => {
            request_by_map(&ctx, &orig, id, user_id, mods.as_ref(), legacy_scores).await
        }
        Some(MapOrScore::Map(MapIdType::Set(_))) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
        None if orig.can_read_history() => {
            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            match ctx.find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => {
                    request_by_map(&ctx, &orig, id, user_id, mods.as_ref(), legacy_scores).await
                }
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await;
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            return orig.error(&ctx, content).await;
        }
    };

    let entry = match data_result {
        ScoreResult::Entry(entry) => entry,
        ScoreResult::Done => return Ok(()),
        ScoreResult::Error(err) => return Err(err),
    };

    let embed_data = FixScoreEmbed::new(&entry, mods);
    let builder = embed_data.build().into();
    orig.create_message(&ctx, builder).await?;

    Ok(())
}

// Allow the large variant since it's the most common one
#[allow(clippy::large_enum_variant)]
enum ScoreResult {
    Entry(FixEntry),
    Done,
    Error(Report),
}

pub struct FixEntry {
    pub user: RedisData<User>,
    pub map: OsuMap,
    pub score: Option<FixScore>,
}

pub struct FixScore {
    pub score: ScoreSlim,
    pub top: Vec<Score>,
    pub if_fc: Option<IfFc>,
}

// Retrieve user's score on the map, the user itself, and the map including
// mapset
async fn request_by_map(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    map_id: u32,
    user_id: UserId,
    mods: Option<&GameModsIntermode>,
    legacy_scores: bool,
) -> ScoreResult {
    let map = match ctx.osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?"
            );

            return match orig.error(ctx, content).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            };
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;

            return ScoreResult::Error(err);
        }
    };

    let (user_res, scores_res) = match UserArgs::rosu_id(ctx, &user_id).await.mode(map.mode()) {
        UserArgs::Args(args) => {
            let user_fut = ctx.redis().osu_user_from_args(args);
            let scores_fut = ctx
                .osu_scores()
                .user_on_map(map_id, legacy_scores)
                .exec(args);

            tokio::join!(user_fut, scores_fut)
        }
        UserArgs::User { user, .. } => {
            let args = UserArgsSlim::user_id(user.user_id).mode(map.mode());
            let scores_res = ctx
                .osu_scores()
                .user_on_map(map_id, legacy_scores)
                .exec(args)
                .await;

            (Ok(RedisData::Original(*user)), scores_res)
        }
        UserArgs::Err(err) => (Err(err), Ok(Vec::new())),
    };

    let user = match user_res {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(ctx, user_id).await;

            return match orig.error(ctx, content).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            };
        }
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let wrap = "Failed to get user";

            return ScoreResult::Error(Report::new(err).wrap_err(wrap));
        }
    };

    let score_opt = match scores_res {
        Ok(scores) => match mods {
            None => scores.into_iter().max_by_key(|score| score.ended_at),
            Some(mods) => scores.into_iter().find(|score| {
                let intermode = score
                    .mods
                    .iter()
                    .map(GameMod::intermode)
                    .collect::<GameModsIntermode>();

                &intermode == mods
            }),
        },
        Err(err) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let wrap = "Failed to get scores";

            return ScoreResult::Error(Report::new(err).wrap_err(wrap));
        }
    };

    let score = match score_opt {
        Some(score) => {
            let user_args = UserArgsSlim::user_id(user.user_id()).mode(score.mode);

            let top_fut = ctx
                .osu_scores()
                .top(legacy_scores)
                .limit(100)
                .exec(user_args);

            let pp_fut = async {
                match score.pp {
                    Some(pp) => pp,
                    None => ctx.pp(&map).score(&score).performance().await.pp() as f32,
                }
            };

            let (top_res, pp) = tokio::join!(top_fut, pp_fut);

            let top = match top_res {
                Ok(scores) => scores,
                Err(err) => {
                    let _ = orig.error(ctx, OSU_API_ISSUE).await;
                    let wrap = "failed to get top scores";

                    return ScoreResult::Error(Report::new(err).wrap_err(wrap));
                }
            };

            let score = ScoreSlim::new(score, pp);

            // Not being done concurrently with the previous two because
            // then the map retrieval might happen twice
            let if_fc = IfFc::new(ctx, &score, &map).await;

            Some(FixScore { score, top, if_fc })
        }
        None => None,
    };

    ScoreResult::Entry(FixEntry { user, map, score })
}

async fn request_by_score(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    score_id: u64,
    mode: GameMode,
    user_id: UserId,
    legacy_scores: bool,
) -> ScoreResult {
    let score_fut = ctx.osu().score(score_id, mode);
    let user_args = UserArgs::rosu_id(ctx, &user_id).await.mode(mode);
    let user_fut = ctx.redis().osu_user(user_args);

    let (user, score) = match tokio::join!(user_fut, score_fut) {
        (Ok(user), Ok(score)) => (user, score),
        (Err(OsuError::NotFound), _) => {
            let content = user_not_found(ctx, user_id).await;

            return match orig.error(ctx, content).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            };
        }
        (_, Err(OsuError::NotFound)) => {
            let content = format!("A score with id {score_id} does not exists");

            return match orig.error(ctx, content).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            };
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return ScoreResult::Error(err);
        }
    };

    let map = score.map.as_ref().expect("missing map");
    let map_id = score.map_id;

    let map_fut = ctx.osu_map().map(map_id, map.checksum.as_deref());
    let user_args = UserArgsSlim::user_id(score.user_id).mode(score.mode);
    let best_fut = ctx
        .osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec(user_args);

    let (map, top) = match tokio::join!(map_fut, best_fut) {
        (Ok(map), Ok(best)) => (map, best),
        (Err(MapError::NotFound), _) => {
            let content = format!("There is no map with id {map_id}");

            return match orig.error(ctx, content).await {
                Ok(_) => ScoreResult::Done,
                Err(err) => ScoreResult::Error(err),
            };
        }
        (Err(MapError::Report(err)), _) => {
            let _ = orig.error(ctx, GENERAL_ISSUE).await;

            return ScoreResult::Error(err);
        }
        (_, Err(err)) => {
            let _ = orig.error(ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get top scores");

            return ScoreResult::Error(err);
        }
    };

    let pp = match score.pp {
        Some(pp) => pp,
        None => ctx.pp(&map).score(&score).performance().await.pp() as f32,
    };

    let score = ScoreSlim::new(score, pp);
    let if_fc = IfFc::new(ctx, &score, &map).await;

    let data = FixEntry {
        user,
        map,
        score: Some(FixScore { score, top, if_fc }),
    };

    ScoreResult::Entry(data)
}
