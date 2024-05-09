use std::{
    borrow::Cow,
    cmp::Reverse,
    collections::{hash_map::Entry, HashMap},
};

use bathbot_macros::{command, HasMods, SlashCommand};
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::{AVATAR_URL, GENERAL_ISSUE, OSU_WEB_ISSUE},
    matcher,
    osu::{MapIdType, ModSelection},
    IntHasher,
};
use eyre::{Report, Result};
use rosu_pp::any::{DifficultyAttributes, ScoreState};
use rosu_v2::{
    model::score::LegacyScoreStatistics,
    prelude::{
        BeatmapUserScore, GameMode, GameMods, GameModsIntermode, Grade, OsuError, Score, Username,
    },
};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
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
        MapError, Mods, OsuMap,
    },
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
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
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `score`.\n\
        Note that the scores will still be the top pp scores, they'll just be re-ordered."
    )]
    sort: Option<LeaderboardSort>,
}

#[derive(Copy, Clone, Default, CommandOption, CreateOption, Eq, PartialEq)]
pub enum LeaderboardSort {
    #[option(name = "Accuracy", value = "acc")]
    Accuracy,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Misses", value = "misses")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[default]
    #[option(name = "Score", value = "score")]
    Score,
}

pub type AttrMap = HashMap<Mods, (DifficultyAttributes, f32)>;

impl LeaderboardSort {
    pub async fn sort(self, scores: &mut [LeaderboardScore], map: &OsuMap, attr_map: &mut AttrMap) {
        match self {
            Self::Accuracy => scores.sort_by(|a, b| b.accuracy.total_cmp(&a.accuracy)),
            Self::Combo => scores.sort_by_key(|score| Reverse(score.combo)),
            Self::Date => scores.sort_by_key(|score| score.ended_at),
            Self::Misses => scores.sort_by_key(|score| score.statistics.count_miss),
            Self::Pp => {
                let mut pps = HashMap::with_capacity_and_hasher(scores.len(), IntHasher);

                for score in scores.iter() {
                    let (pp, _) = score.pp(map, attr_map).await;
                    pps.insert(score.pos, pp);
                }

                scores.sort_by(|a, b| {
                    let a_pp = pps.get(&a.pos).copied().unwrap_or(0.0);
                    let b_pp = pps.get(&b.pos).copied().unwrap_or(0.0);

                    b_pp.total_cmp(&a_pp)
                })
            }
            Self::Score => scores.sort_by_key(|score| Reverse(score.score)),
        }
    }

    pub fn push_content(self, content: &mut String) {
        match self {
            Self::Accuracy => content.push_str(" (`Order: Accuracy`)"),
            Self::Combo => content.push_str(" (`Order: Combo`)"),
            Self::Date => content.push_str(" (`Order: Date`)"),
            Self::Misses => content.push_str(" (`Order: Misses`)"),
            Self::Pp => content.push_str(" (`Order: PP`)"),
            Self::Score => content.push_str(" (`Order: Score`)"),
        }
    }
}

#[derive(HasMods)]
struct LeaderboardArgs<'a> {
    map: Option<MapIdType>,
    mods: Option<Cow<'a, str>>,
    sort: LeaderboardSort,
}

impl<'m> LeaderboardArgs<'m> {
    async fn args(msg: &Message, args: Args<'m>) -> Result<LeaderboardArgs<'m>, String> {
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
            if let Some(id) = Context::find_map_id_in_msg(reply).await {
                map = Some(id);
            }
        }

        let sort = LeaderboardSort::default();

        Ok(Self { map, mods, sort })
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
            sort: args.sort.unwrap_or_default(),
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
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match LeaderboardArgs::args(msg, args).await {
        Ok(args) => leaderboard(CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_leaderboard(mut command: InteractionCommand) -> Result<()> {
    let args = Leaderboard::from_interaction(command.input_data())?;

    match LeaderboardArgs::try_from(args) {
        Ok(args) => leaderboard((&mut command).into(), args).await,
        Err(content) => {
            command.error(content).await?;

            Ok(())
        }
    }
}

async fn leaderboard(orig: CommandOrigin<'_>, args: LeaderboardArgs<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(content).await;
        }
    };

    let owner = orig.user_id()?;

    let map_id_fut = get_map_id(&orig, args.map);
    let config_fut = Context::user_config().with_osu_id(owner);

    let (map_id_res, config_res) = tokio::join!(map_id_fut, config_fut);

    let map_id = match map_id_res {
        Ok(map_id) => map_id,
        Err(content) => return orig.error(content).await,
    };

    let config = config_res?;

    // Retrieving the beatmap
    let map = match Context::osu_map().map(map_id, None).await {
        Ok(map) => map,
        Err(MapError::NotFound) => {
            let content = format!(
                "Could not find beatmap with id `{map_id}`. \
                Did you give me a mapset id instead of a map id?",
            );

            return orig.error(content).await;
        }
        Err(MapError::Report(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let legacy_scores = match config.legacy_scores {
        Some(legacy_scores) => legacy_scores,
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.legacy_scores)
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let specify_mods = match mods {
        Some(ModSelection::Include(ref mods) | ModSelection::Exact(ref mods)) => {
            Some(mods.to_owned())
        }
        Some(ModSelection::Exclude(_)) | None => None,
    };

    let mods_bits = specify_mods.as_ref().map_or(0, GameModsIntermode::bits);

    let mut calc = Context::pp(&map)
        .mode(map.mode())
        .mods(Mods::new(mods_bits));
    let attrs_fut = calc.performance();

    let scores_fut = Context::osu_scores().map_leaderboard(
        map_id,
        map.mode(),
        specify_mods.clone(),
        100,
        legacy_scores,
    );

    let user_fut = get_user_score(
        config.osu,
        map_id,
        map.mode(),
        specify_mods.clone(),
        legacy_scores,
    );

    let (scores_res, user_res, attrs) = tokio::join!(scores_fut, user_fut, attrs_fut);

    let mut scores: Vec<_> = match scores_res {
        Ok(scores) => scores
            .into_iter()
            .enumerate()
            .map(|(i, mut score)| {
                let user = score.user.take();

                LeaderboardScore::new(
                    score.user_id,
                    user.map_or_else(|| "<unknown user>".into(), |user| user.username),
                    score,
                    i + 1,
                )
            })
            .collect(),
        Err(err) => {
            let _ = orig.error(OSU_WEB_ISSUE).await;

            return Err(err.wrap_err("Failed to get leaderboard"));
        }
    };

    let mut user_score = user_res
        .unwrap_or_else(|err| {
            warn!(?err, "Failed to get user score");

            None
        })
        .map(|(user, score)| LeaderboardUserScore {
            discord_id: owner,
            score: LeaderboardScore::new(
                user.user_id(),
                user.username().into(),
                score.score,
                score.pos,
            ),
        });

    let amount = scores.len();

    if let Some(ModSelection::Exclude(ref mods)) = mods {
        if mods.is_empty() {
            scores.retain(|score| !score.mods.is_empty());

            if let Some(ref score) = user_score {
                if score.score.mods.is_empty() {
                    user_score.take();
                }
            }
        } else {
            scores.retain(|score| !score.mods.contains_any(mods.iter()));

            if let Some(ref score) = user_score {
                if score.score.mods.contains_any(mods.iter()) {
                    user_score.take();
                }
            }
        }
    }

    let mut content = if mods.is_some() {
        format!("I found {amount} scores with the specified mods on the map's leaderboard")
    } else {
        format!("I found {amount} scores on the map's leaderboard")
    };

    let stars = attrs.stars() as f32;
    let max_combo = attrs.max_combo();

    // Not storing `attrs` here in case mods (potentially with clock rate) were
    // specified
    let mut attr_map = HashMap::default();

    args.sort.sort(&mut scores, &map, &mut attr_map).await;
    args.sort.push_content(&mut content);

    let first_place_icon = scores.first().map(|s| format!("{AVATAR_URL}{}", s.user_id));

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
        .begin(orig)
        .await
}

async fn get_map_id(orig: &CommandOrigin<'_>, map: Option<MapIdType>) -> Result<u32, &'static str> {
    match map {
        Some(MapIdType::Map(id)) => Ok(id),
        Some(MapIdType::Set(_)) => {
            Err("Looks like you gave me a mapset id, I need a map id though")
        }
        None => {
            let msgs = Context::retrieve_channel_history(orig.channel_id())
                .await
                .map_err(|_| {
                    "No beatmap specified and lacking permission to search the channel \
                    history for maps.\nTry specifying a map either by url to the map, or \
                    just by map id, or give me the \"Read Message History\" permission."
                })?;

            match Context::find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => Ok(id),
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                        Try specifying a map either by url to the map, or just by map id.";

                    Err(content)
                }
            }
        }
    }
}

async fn get_user_score(
    osu_id: Option<u32>,
    map_id: u32,
    mode: GameMode,
    mods: Option<GameModsIntermode>,
    legacy_scores: bool,
) -> Result<Option<(RedisData<User>, BeatmapUserScore)>> {
    let Some(user_id) = osu_id else {
        return Ok(None);
    };

    let user_args = UserArgs::user_id(user_id).mode(mode);
    let user_fut = Context::redis().osu_user(user_args);

    let score_fut =
        Context::osu_scores().user_on_map_single(user_id, map_id, mode, mods, legacy_scores);

    match tokio::try_join!(user_fut, score_fut) {
        Ok(tuple) => Ok(Some(tuple)),
        Err(OsuError::NotFound) => Ok(None),
        Err(err) => Err(Report::new(err).wrap_err("Failed to get score or user")),
    }
}

pub struct LeaderboardScore {
    pub user_id: u32,
    pub username: Username,
    pub pos: usize,
    pub grade: Grade,
    pub accuracy: f32,
    pub statistics: LegacyScoreStatistics,
    pub mode: GameMode,
    pub mods: GameMods,
    pub combo: u32,
    pub score: u32,
    pub ended_at: OffsetDateTime,
}

impl LeaderboardScore {
    pub fn new(user_id: u32, username: Username, score: Score, pos: usize) -> Self {
        Self {
            user_id,
            username,
            pos,
            grade: if score.passed { score.grade } else { Grade::F },
            accuracy: score.accuracy,
            statistics: score.statistics.as_legacy(score.mode),
            mode: score.mode,
            mods: score.mods,
            combo: score.max_combo,
            score: score.score,
            ended_at: score.ended_at,
        }
    }
}

impl LeaderboardScore {
    pub async fn pp(&self, map: &OsuMap, attr_map: &mut AttrMap) -> (f32, f32) {
        let mods = Mods::from(&self.mods);

        match attr_map.entry(mods) {
            Entry::Occupied(entry) => {
                let (attrs, max_pp) = entry.get();

                let state = ScoreState {
                    max_combo: self.combo,
                    n_geki: self.statistics.count_geki,
                    n_katu: self.statistics.count_katu,
                    n300: self.statistics.count_300,
                    n100: self.statistics.count_100,
                    n50: self.statistics.count_50,
                    misses: self.statistics.count_miss,
                };

                let mut pp_calc = attrs.to_owned().performance().mods(mods.bits).state(state);

                if let Some(clock_rate) = mods.clock_rate {
                    pp_calc = pp_calc.clock_rate(f64::from(clock_rate));
                }

                let pp = pp_calc.calculate().pp() as f32;

                (pp, *max_pp)
            }
            Entry::Vacant(entry) => {
                let mut calc = Context::pp(map).mode(self.mode).mods(mods);
                let attrs = calc.performance().await;
                let max_pp = attrs.pp() as f32;
                let pp = calc.score(self).performance().await.pp() as f32;
                entry.insert((attrs.into(), max_pp));

                (pp, max_pp)
            }
        }
    }
}

pub struct LeaderboardUserScore {
    pub discord_id: Id<UserMarker>,
    pub score: LeaderboardScore,
}
