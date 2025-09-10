use std::{borrow::Cow, cmp::Reverse, collections::HashMap};

use bathbot_macros::{HasMods, SlashCommand, command};
use bathbot_model::command_fields::GameModeOption;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    IntHasher, ScoreExt,
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::{MapIdType, ModSelection},
};
use eyre::{Report, Result};
use rosu_v2::prelude::{
    BeatmapUserScore, GameMode, GameMods, GameModsIntermode, Grade, OsuError, Score,
    ScoreStatistics, Username,
};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    channel::Message,
    guild::Permissions,
    id::{Id, marker::UserMarker},
};

use super::{HasMods, ModsResult};
use crate::{
    Context,
    active::{ActiveMessages, impls::LeaderboardPagination},
    commands::utility::{SCORE_DATA_DESC, SCORE_DATA_HELP},
    core::commands::{CommandOrigin, prefix::Args},
    manager::{
        MapError, Mods, OsuMap,
        redis::osu::{CachedUser, UserArgs, UserArgsError},
    },
    util::{ChannelExt, InteractionCommandExt, interaction::InteractionCommand, osu::MapOrScore},
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
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `score`.\n\
        Note that the scores will still be the top pp scores, they'll just be re-ordered."
    )]
    sort: Option<LeaderboardSort>,
    #[command(desc = SCORE_DATA_DESC, help = SCORE_DATA_HELP)]
    score_data: Option<ScoreData>,
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

impl LeaderboardSort {
    pub async fn sort(self, scores: &mut [LeaderboardScore], map: &OsuMap, score_data: ScoreData) {
        match self {
            Self::Accuracy => scores.sort_by(|a, b| b.accuracy.total_cmp(&a.accuracy)),
            Self::Combo => scores.sort_by_key(|score| Reverse(score.combo)),
            Self::Date => scores.sort_by_key(|score| score.ended_at),
            Self::Misses => scores.sort_by_key(|score| score.statistics.miss),
            Self::Pp => {
                let mut pps = HashMap::with_capacity_and_hasher(scores.len(), IntHasher);

                for score in scores.iter_mut() {
                    let pp = score.pp(map).await.pp;
                    pps.insert(score.pos, pp);
                }

                scores.sort_by(|a, b| {
                    let a_pp = pps.get(&a.pos).copied().unwrap_or(0.0);
                    let b_pp = pps.get(&b.pos).copied().unwrap_or(0.0);

                    b_pp.total_cmp(&a_pp)
                })
            }
            Self::Score if score_data == ScoreData::LazerWithClassicScoring => {
                scores.sort_by_key(|score| Reverse(score.classic_score))
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
    mode: Option<GameMode>,
    sort: LeaderboardSort,
    score_data: Option<ScoreData>,
}

impl<'m> LeaderboardArgs<'m> {
    async fn args(
        msg: &Message,
        args: Args<'m>,
        mode: Option<GameMode>,
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

        if map.is_none() {
            match MapOrScore::find_in_msg(msg).await {
                Some(MapOrScore::Map(id)) => map = Some(id),
                Some(MapOrScore::Score { .. }) => {
                    return Err(
                        "This command does not (yet) accept score urls as argument".to_owned()
                    );
                }
                None => {}
            }
        }

        let sort = LeaderboardSort::default();

        Ok(Self {
            map,
            mods,
            mode,
            sort,
            score_data: None,
        })
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
            mode: args.mode.map(GameMode::from),
            sort: args.sort.unwrap_or_default(),
            score_data: args.score_data,
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
    match LeaderboardArgs::args(msg, args, None).await {
        Ok(args) => leaderboard(CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the global leaderboard of a taiko map")]
#[help(
    "Display the global leaderboard of a given taiko map.\n\
    If no map is given, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[alias("lbt")]
#[group(Taiko)]
async fn prefix_leaderboardtaiko(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match LeaderboardArgs::args(msg, args, Some(GameMode::Taiko)).await {
        Ok(args) => leaderboard(CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the global leaderboard of a catch map")]
#[help(
    "Display the global leaderboard of a given catch map.\n\
    If no map is given, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[alias("lbc", "leaderboardcatch")]
#[group(Catch)]
async fn prefix_leaderboardctb(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match LeaderboardArgs::args(msg, args, Some(GameMode::Catch)).await {
        Ok(args) => leaderboard(CommandOrigin::from_msg(msg, permissions), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display the global leaderboard of a mania map")]
#[help(
    "Display the global leaderboard of a given mania map.\n\
    If no map is given, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[alias("lbm")]
#[group(Mania)]
async fn prefix_leaderboardmania(
    msg: &Message,
    args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match LeaderboardArgs::args(msg, args, Some(GameMode::Mania)).await {
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
        Ok(mut map) => {
            if let Some(mode) = args.mode {
                map.convert_mut(mode);
            }

            map
        }
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

    let score_data = match args.score_data.or(config.score_data) {
        Some(score_data) => score_data,
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .unwrap_or_default(),
            None => Default::default(),
        },
    };

    let legacy_scores = score_data.is_legacy();

    let specify_mods = match mods {
        Some(ModSelection::Include(ref mods) | ModSelection::Exact(ref mods)) => {
            Some(mods.to_owned())
        }
        None | Some(ModSelection::Exclude { .. }) => None,
    };

    let mods_ = specify_mods
        .as_ref()
        .map_or_else(GameModsIntermode::default, GameModsIntermode::to_owned);

    let mode = map.mode();

    let mut calc = Context::pp(&map).mode(mode).mods(Mods::new(mods_));
    let attrs_fut = calc.performance();

    const SCORE_COUNT: usize = 100;

    let scores_fut = Context::osu_scores().map_leaderboard(
        map_id,
        mode,
        specify_mods.clone(),
        SCORE_COUNT as u32,
        legacy_scores,
    );

    let user_fut = get_user_score(
        config.osu,
        map_id,
        mode,
        specify_mods.clone(),
        legacy_scores,
    );

    let (scores_res, user_res, attrs) = tokio::join!(scores_fut, user_fut, attrs_fut);

    let mut avatar_urls = HashMap::with_capacity_and_hasher(SCORE_COUNT, IntHasher);

    let mut scores: Vec<_> = match scores_res {
        Ok(scores) => scores
            .into_iter()
            .enumerate()
            .map(|(i, mut score)| {
                let username = match score.user.take() {
                    Some(user) => {
                        avatar_urls.insert(score.id, user.avatar_url.into_boxed_str());

                        user.username
                    }
                    None => format!("<user {}>", score.user_id).into(),
                };

                LeaderboardScore::new(score.user_id, username, score, i + 1)
            })
            .collect(),
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;

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
                user.user_id.to_native(),
                user.username.as_str().into(),
                score.score,
                score.pos,
            ),
        });

    if let Some(ModSelection::Exclude { ref mods, nomod }) = mods {
        scores.retain(|score| ModSelection::filter_exclude(mods, nomod, &score.mods));

        if let Some(ref score) = user_score
            && ModSelection::filter_exclude(mods, nomod, &score.score.mods)
        {
            user_score.take();
        }
    }

    let amount = scores.len();

    let mut content = if mods.is_some() {
        format!("I found {amount} scores with the specified mods on the map's leaderboard")
    } else {
        format!("I found {amount} scores on the map's leaderboard")
    };

    let mut stars = 0.0;
    let mut max_combo = 0;

    if let Some(attrs) = attrs {
        stars = attrs.stars() as f32;
        max_combo = attrs.max_combo();
    }

    args.sort.sort(&mut scores, &map, score_data).await;
    args.sort.push_content(&mut content);

    let first_place_icon = scores.first().and_then(|s| avatar_urls.remove(&s.score_id));

    let pagination = LeaderboardPagination::builder()
        .map(map)
        .scores(scores.into_boxed_slice())
        .stars(stars)
        .max_combo(max_combo)
        .author_data(user_score)
        .first_place_icon(first_place_icon)
        .score_data(score_data)
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
) -> Result<Option<(CachedUser, BeatmapUserScore)>> {
    let Some(user_id) = osu_id else {
        return Ok(None);
    };

    let user_args = UserArgs::user_id(user_id, mode);
    let user_fut = Context::redis().osu_user(user_args);

    let score_fut =
        Context::osu_scores().user_on_map_single(user_id, map_id, mode, mods, legacy_scores);

    let (user_res, score_res) = tokio::join!(user_fut, score_fut);

    let user = match user_res {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => return Ok(None),
        Err(err) => return Err(Report::new(err).wrap_err("Failed to get user")),
    };

    let score = match score_res {
        Ok(score) => score,
        Err(OsuError::NotFound) => return Ok(None),
        Err(err) => return Err(Report::new(err).wrap_err("Failed to get score")),
    };

    Ok(Some((user, score)))
}

pub struct LeaderboardScore {
    pub user_id: u32,
    pub username: Username,
    pub pos: usize,
    pub grade: Grade,
    pub accuracy: f32,
    pub statistics: ScoreStatistics,
    pub mode: GameMode,
    pub mods: GameMods,
    pub combo: u32,
    pub score: u32,
    pub classic_score: u64,
    pub ended_at: OffsetDateTime,
    pub score_id: u64,
    pub is_legacy: bool,
    pub set_on_lazer: bool,
    pub pps: Option<PpData>,
}

impl LeaderboardScore {
    pub fn new(user_id: u32, username: Username, score: Score, pos: usize) -> Self {
        Self {
            user_id,
            username,
            pos,
            is_legacy: score.is_legacy(),
            set_on_lazer: score.set_on_lazer,
            grade: if score.passed { score.grade } else { Grade::F },
            accuracy: score.accuracy,
            statistics: score.statistics,
            mode: score.mode,
            mods: score.mods,
            combo: score.max_combo,
            score: score.score,
            classic_score: score.classic_score,
            ended_at: score.ended_at,
            score_id: score.id,
            pps: None,
        }
    }
}

#[derive(Copy, Clone)]
pub struct PpData {
    pub pp: f32,
    pub max: f32,
}

impl LeaderboardScore {
    pub async fn pp(&mut self, map: &OsuMap) -> PpData {
        if let Some(pps) = self.pps {
            return pps;
        }

        let mut calc = Context::pp(map)
            .mode(self.mode)
            .mods(self.mods.clone())
            .lazer(self.set_on_lazer);

        let mut max_pp = 0.0;
        let mut pp = 0.0;

        if let Some(max_attrs) = calc.performance().await {
            max_pp = max_attrs.pp() as f32;

            if let Some(attrs) = calc.score(&*self).performance().await {
                pp = attrs.pp() as f32;
            }
        }

        *self.pps.insert(PpData { pp, max: max_pp })
    }
}

pub struct LeaderboardUserScore {
    pub discord_id: Id<UserMarker>,
    pub score: LeaderboardScore,
}
