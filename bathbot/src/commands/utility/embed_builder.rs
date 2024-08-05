use std::sync::Arc;

use bathbot_macros::SlashCommand;
use bathbot_model::ScoreSlim;
use bathbot_util::{constants::GENERAL_ISSUE, CowUtils, MessageOrigin};
use eyre::{Report, Result};
use rosu_pp::model::beatmap::BeatmapAttributes;
use rosu_v2::{
    model::{GameMode, Grade},
    prelude::{GameModIntermode, GameMods, LegacyScoreStatistics, RankStatus, Score},
};
use time::OffsetDateTime;
use twilight_interactions::command::CreateCommand;
use twilight_model::id::{marker::GuildMarker, Id};

use crate::{
    active::{impls::ScoreEmbedBuilderActive, ActiveMessages},
    core::Context,
    manager::{redis::osu::UserArgsSlim, MapError, OsuMap, OwnedReplayScore, PpManager},
    util::{
        interaction::InteractionCommand,
        osu::{IfFc, PersonalBestIndex},
        query::{FilterCriteria, Searchable, TopCriteria},
        Authored, InteractionCommandExt,
    },
};

const USER_ID: u32 = 2;
const MAP_ID: u32 = 197337;
const MAP_CHECKSUM: &str = "a708a5b90349e98b399f2a1c9fce5422";

#[derive(CreateCommand, SlashCommand)]
#[command(name = "builder", desc = "Build your own score embed format")]
pub struct ScoreEmbedBuilder;

pub async fn slash_scoreembedbuilder(mut command: InteractionCommand) -> Result<()> {
    let msg_owner = command.user_id()?;

    let config = match Context::user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let score_data = match config.score_data {
        Some(score_data) => score_data,
        None => match command.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .unwrap_or_default(),
            None => Default::default(),
        },
    };

    let legacy_scores = score_data.is_legacy();

    let user_fut = Context::redis().osu_user_from_args(UserArgsSlim::user_id(USER_ID));

    let score_fut = Context::osu_scores().user_on_map_single(
        USER_ID,
        MAP_ID,
        GameMode::Osu,
        None,
        legacy_scores,
    );

    let map_fut = Context::osu_map().map(MAP_ID, Some(MAP_CHECKSUM));

    let (user, score, map) = match tokio::join!(user_fut, score_fut, map_fut) {
        (Ok(user), Ok(score), Ok(map)) => (user, score.score, map),
        (user_res, score_res, map_res) => {
            let _ = command.error(GENERAL_ISSUE).await;

            let (err, wrap) = if let Err(err) = user_res {
                (Report::new(err), "Failed to get user for builder")
            } else if let Err(err) = score_res {
                (Report::new(err), "Failed to get score for builder")
            } else if let Err(err) = map_res {
                (Report::new(err), "Failed to get map for builder")
            } else {
                unreachable!()
            };

            return Err(err.wrap_err(wrap));
        }
    };

    let settings = config.score_embed.unwrap_or_default();

    let data = ScoreEmbedDataWrap::new_custom(score, map, 71, Some(7)).await;

    let active_msg = ScoreEmbedBuilderActive::new(&user, data, settings, score_data, msg_owner);

    ActiveMessages::builder(active_msg)
        .start_by_update(true)
        .begin(&mut command)
        .await
}

pub struct ScoreEmbedDataWrap {
    inner: ScoreEmbedDataStatus,
}

impl ScoreEmbedDataWrap {
    /// Create a [`ScoreEmbedDataWrap`] with a [`Score`] and only some
    /// metadata.
    pub fn new_raw(
        score: Score,
        legacy_scores: bool,
        with_render: bool,
        miss_analyzer: MissAnalyzerCheck,
        top100: Option<Arc<[Score]>>,
        #[cfg(feature = "twitch")] twitch_data: Option<Arc<TwitchData>>,
        origin: MessageOrigin,
    ) -> Self {
        Self {
            inner: ScoreEmbedDataStatus::Raw(Some(ScoreEmbedDataRaw::new(
                score,
                legacy_scores,
                with_render,
                miss_analyzer,
                top100,
                #[cfg(feature = "twitch")]
                twitch_data,
                origin,
            ))),
        }
    }

    /// Create a [`ScoreEmbedDataWrap`] with a [`Score`], an [`OsuMap`], and
    /// only some metadata.
    pub async fn new_half(
        score: Score,
        map: OsuMap,
        checksum: Option<String>,
        pb_idx: Option<ScoreEmbedDataPersonalBest>,
        legacy_scores: bool,
        with_render: bool,
        miss_analyzer: MissAnalyzerCheck,
    ) -> Self {
        Self {
            inner: ScoreEmbedDataStatus::Half(Some(
                ScoreEmbedDataHalf::new(
                    score,
                    map,
                    checksum,
                    pb_idx,
                    legacy_scores,
                    with_render,
                    miss_analyzer,
                )
                .await,
            )),
        }
    }

    pub async fn new_custom(
        score: Score,
        map: OsuMap,
        pb_idx: usize,
        global_idx: Option<usize>,
    ) -> Self {
        let PpAttrs {
            calc,
            stars,
            max_combo,
            max_pp,
        } = PpAttrs::new(&map, score.mode, &score.mods, score.grade, score.pp).await;

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);

        let if_fc_pp = IfFc::new(&score, &map).await.map(|if_fc| if_fc.pp);

        Self {
            inner: ScoreEmbedDataStatus::Full(ScoreEmbedData {
                score,
                map,
                stars,
                max_combo,
                max_pp,
                replay: None,
                miss_analyzer: None,
                pb_idx: Some(ScoreEmbedDataPersonalBest::from_index(pb_idx)),
                global_idx,
                if_fc_pp,
                #[cfg(feature = "twitch")]
                twitch: None,
            }),
        }
    }

    /// Returns the inner [`ScoreEmbedData`].
    ///
    /// If the data has not yet been calculated, it will do so first.
    pub async fn get_mut(&mut self) -> Result<&mut ScoreEmbedData> {
        let data = match self.inner {
            ScoreEmbedDataStatus::Raw(ref mut raw) => {
                let raw = raw
                    .take()
                    .ok_or_else(|| eyre!("Raw data was already taken"))?;

                self.inner = ScoreEmbedDataStatus::Empty;

                raw.into_full().await?
            }
            ScoreEmbedDataStatus::Half(ref mut half) => {
                let half = half
                    .take()
                    .ok_or_else(|| eyre!("Half data was already taken"))?;

                self.inner = ScoreEmbedDataStatus::Empty;

                half.into_full().await
            }
            ScoreEmbedDataStatus::Full(ref mut data) => return Ok(data),
            ScoreEmbedDataStatus::Empty => bail!("Empty data"),
        };

        self.inner = ScoreEmbedDataStatus::Full(data);

        let ScoreEmbedDataStatus::Full(ref mut data) = self.inner else {
            unreachable!()
        };

        Ok(data)
    }

    /// Returns the inner [`ScoreEmbedData`].
    ///
    /// If the data has not yet been calculated, returns `None`.
    pub fn try_get(&self) -> Option<&ScoreEmbedData> {
        self.inner.try_get()
    }

    pub fn try_get_half(&self) -> Option<&ScoreEmbedDataHalf> {
        if let ScoreEmbedDataStatus::Half(ref half) = self.inner {
            half.as_ref()
        } else {
            None
        }
    }

    #[track_caller]
    pub fn get_half(&self) -> &ScoreEmbedDataHalf {
        self.try_get_half().unwrap()
    }
}

impl From<ScoreEmbedDataHalf> for ScoreEmbedDataWrap {
    fn from(data: ScoreEmbedDataHalf) -> Self {
        Self {
            inner: ScoreEmbedDataStatus::Half(Some(data)),
        }
    }
}

enum ScoreEmbedDataStatus {
    Raw(Option<ScoreEmbedDataRaw>),
    Half(Option<ScoreEmbedDataHalf>),
    Full(ScoreEmbedData),
    Empty,
}

impl ScoreEmbedDataStatus {
    fn try_get(&self) -> Option<&ScoreEmbedData> {
        match self {
            Self::Full(ref data) => Some(data),
            Self::Raw(_) | Self::Half(_) | Self::Empty => None,
        }
    }
}

pub struct ScoreEmbedDataHalf {
    pub user_id: u32,
    pub checksum: Option<String>,
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub stars: f32,
    pub max_combo: u32,
    pub max_pp: f32,
    pub pb_idx: Option<ScoreEmbedDataPersonalBest>,
    pub legacy_scores: bool,
    pub with_render: bool,
    pub miss_analyzer_check: MissAnalyzerCheck,
    pub original_idx: Option<usize>,
}

impl ScoreEmbedDataHalf {
    pub async fn new(
        score: Score,
        map: OsuMap,
        checksum: Option<String>,
        pb_idx: Option<ScoreEmbedDataPersonalBest>,
        legacy_scores: bool,
        with_render: bool,
        miss_analyzer_check: MissAnalyzerCheck,
    ) -> Self {
        let user_id = score.user_id;

        let PpAttrs {
            calc,
            stars,
            max_combo,
            max_pp,
        } = PpAttrs::new(&map, score.mode, &score.mods, score.grade, score.pp).await;

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);

        Self {
            user_id,
            checksum,
            score,
            map,
            stars,
            max_combo,
            max_pp,
            pb_idx,
            legacy_scores,
            with_render,
            miss_analyzer_check,
            original_idx: None,
        }
    }

    async fn into_full(self) -> ScoreEmbedData {
        let global_idx_fut = async {
            if !matches!(
                self.map.status(),
                RankStatus::Ranked
                    | RankStatus::Loved
                    | RankStatus::Qualified
                    | RankStatus::Approved
            ) || self.score.grade == Grade::F
            {
                return None;
            }

            let map_lb_fut = Context::osu_scores().map_leaderboard(
                self.map.map_id(),
                self.score.mode,
                None,
                50,
                self.legacy_scores,
            );

            let scores = match map_lb_fut.await {
                Ok(scores) => scores,
                Err(err) => {
                    warn!(?err, "Failed to get global scores");

                    return None;
                }
            };

            scores
                .iter()
                .position(|s| s.user_id == self.user_id && self.score.is_eq(s))
                .map(|idx| idx + 1)
        };

        let miss_analyzer_fut = async {
            let (score_id, guild_id) = self
                .score
                .legacy_id
                .zip(self.miss_analyzer_check.guild_id)?;

            debug!(score_id, "Sending score id to miss analyzer");

            match Context::client()
                .miss_analyzer_score_request(guild_id.get(), score_id)
                .await
            {
                Ok(wants_button) => wants_button.then_some(MissAnalyzerData { score_id }),
                Err(err) => {
                    warn!(?err, "Failed to send score id to miss analyzer");

                    None
                }
            }
        };

        let if_fc_fut = IfFc::new(&self.score, &self.map);

        let (global_idx, if_fc, miss_analyzer) =
            tokio::join!(global_idx_fut, if_fc_fut, miss_analyzer_fut);

        let if_fc_pp = if_fc.map(|if_fc| if_fc.pp);

        let replay = if self.with_render {
            self.checksum
                .map(String::into_boxed_str)
                .and_then(|checksum| {
                    OwnedReplayScore::try_from_slim(&self.score, self.max_combo, checksum)
                })
        } else {
            None
        };

        ScoreEmbedData {
            score: self.score,
            map: self.map,
            stars: self.stars,
            max_combo: self.max_combo,
            max_pp: self.max_pp,
            replay,
            miss_analyzer,
            pb_idx: self.pb_idx,
            global_idx,
            if_fc_pp,
            #[cfg(feature = "twitch")]
            twitch: None,
        }
    }

    fn map_attrs(&self) -> BeatmapAttributes {
        self.map.attributes().mods(self.score.mods.clone()).build()
    }

    pub fn ar(&self) -> f64 {
        self.map_attrs().ar
    }

    pub fn cs(&self) -> f64 {
        self.map_attrs().cs
    }

    pub fn hp(&self) -> f64 {
        self.map_attrs().hp
    }

    pub fn od(&self) -> f64 {
        self.map_attrs().od
    }
}

pub struct ScoreEmbedData {
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub stars: f32,
    pub max_combo: u32,
    pub max_pp: f32,
    pub replay: Option<OwnedReplayScore>,
    pub miss_analyzer: Option<MissAnalyzerData>,
    pub pb_idx: Option<ScoreEmbedDataPersonalBest>,
    pub global_idx: Option<usize>,
    pub if_fc_pp: Option<f32>,
    #[cfg(feature = "twitch")]
    pub twitch: Option<Arc<TwitchData>>,
}

impl ScoreEmbedData {
    fn map_attrs(&self) -> BeatmapAttributes {
        self.map.attributes().mods(self.score.mods.clone()).build()
    }

    pub fn ar(&self) -> f64 {
        self.map_attrs().ar
    }

    pub fn cs(&self) -> f64 {
        self.map_attrs().cs
    }

    pub fn hp(&self) -> f64 {
        self.map_attrs().hp
    }

    pub fn od(&self) -> f64 {
        self.map_attrs().od
    }
}

#[cfg(feature = "twitch")]
pub enum TwitchData {
    Vod {
        vod: bathbot_model::TwitchVideo,
        stream_login: Box<str>,
    },
    Stream {
        login: Box<str>,
    },
}

#[cfg(feature = "twitch")]
impl TwitchData {
    pub fn append_to_description(&self, score: &ScoreSlim, map: &OsuMap, description: &mut String) {
        match self {
            TwitchData::Vod { vod, stream_login } => {
                let score_start = Self::score_started_at(score, map);
                let vod_start = vod.created_at;
                let vod_end = vod.ended_at();

                if vod_start < score_start && score_start < vod_end {
                    Self::append_vod_to_description(vod, score_start, description);
                } else {
                    Self::append_stream_to_description(stream_login, description);
                }
            }
            TwitchData::Stream { login } => Self::append_stream_to_description(login, description),
        }
    }

    fn score_started_at(score: &ScoreSlim, map: &OsuMap) -> OffsetDateTime {
        let mut map_len = map.seconds_drain() as f64;

        if score.grade == Grade::F {
            // Adjust map length with passed objects in case of fail
            let passed = score.total_hits();

            if map.mode() == GameMode::Catch {
                // Amount objects in .osu file != amount of catch hitobjects
                map_len += 2.0;
            } else if let Some(obj) = passed
                .checked_sub(1)
                .and_then(|i| map.pp_map.hit_objects.get(i as usize))
            {
                map_len = obj.start_time / 1000.0;
            } else {
                let total = map.n_objects();
                map_len *= passed as f64 / total as f64;

                map_len += 2.0;
            }
        } else {
            map_len += map.pp_map.total_break_time() / 1000.0;
        }

        if let Some(clock_rate) = score.mods.clock_rate() {
            map_len /= f64::from(clock_rate);
        }

        score.ended_at - std::time::Duration::from_secs(map_len as u64 + 3)
    }

    fn append_vod_to_description(
        vod: &bathbot_model::TwitchVideo,
        score_start: OffsetDateTime,
        description: &mut String,
    ) {
        use std::fmt::Write;

        let _ = write!(
            description,
            "{emote} [Liveplay on twitch]({url}",
            emote = crate::util::Emote::Twitch,
            url = vod.url
        );

        description.push_str("?t=");
        let mut offset = (score_start - vod.created_at).whole_seconds();

        if offset >= 3600 {
            let _ = write!(description, "{}h", offset / 3600);
            offset %= 3600;
        }

        if offset >= 60 {
            let _ = write!(description, "{}m", offset / 60);
            offset %= 60;
        }

        if offset > 0 {
            let _ = write!(description, "{offset}s");
        }

        description.push(')');
    }

    fn append_stream_to_description(login: &str, description: &mut String) {
        use std::fmt::Write;

        let _ = write!(
            description,
            "{emote} [Streaming on twitch]({base}{login})",
            emote = crate::util::Emote::Twitch,
            base = bathbot_util::constants::TWITCH_BASE
        );
    }
}

pub struct ScoreEmbedDataRaw {
    pub user_id: u32,
    pub map_id: u32,
    pub checksum: Option<String>,
    pub legacy_scores: bool,
    pub with_render: bool,
    pub miss_analyzer_check: MissAnalyzerCheck,
    pub top100: Option<Arc<[Score]>>,
    #[cfg(feature = "twitch")]
    pub twitch: Option<Arc<TwitchData>>,
    pub origin: MessageOrigin,
    pub accuracy: f32,
    pub ended_at: OffsetDateTime,
    pub grade: Grade,
    pub max_combo: u32,
    pub mode: GameMode,
    pub mods: GameMods,
    pub pp: Option<f32>,
    pub score: u32,
    pub classic_score: u32,
    pub score_id: u64,
    pub legacy_id: Option<u64>,
    pub statistics: LegacyScoreStatistics,
    pub has_replay: bool,
}

impl ScoreEmbedDataRaw {
    fn new(
        score: Score,
        legacy_scores: bool,
        with_render: bool,
        miss_analyzer_check: MissAnalyzerCheck,
        top100: Option<Arc<[Score]>>,
        #[cfg(feature = "twitch")] twitch_data: Option<Arc<TwitchData>>,
        origin: MessageOrigin,
    ) -> Self {
        Self {
            user_id: score.user_id,
            map_id: score.map_id,
            checksum: score.map.and_then(|map| map.checksum),
            legacy_scores,
            with_render,
            miss_analyzer_check,
            top100,
            #[cfg(feature = "twitch")]
            twitch: twitch_data,
            origin,
            accuracy: score.accuracy,
            ended_at: score.ended_at,
            grade: if score.passed { score.grade } else { Grade::F },
            max_combo: score.max_combo,
            mode: score.mode,
            mods: score.mods,
            pp: score.pp,
            score: score.score,
            classic_score: score.classic_score,
            score_id: score.id,
            legacy_id: score.legacy_score_id,
            statistics: score.statistics.as_legacy(score.mode),
            has_replay: score.replay,
        }
    }

    async fn into_full(self) -> Result<ScoreEmbedData> {
        let map_id = self.map_id;
        let checksum = self.checksum.as_deref();

        let map_fut = Context::osu_map().map(map_id, checksum);

        let map = match map_fut.await {
            Ok(map) => map.convert(self.mode),
            Err(MapError::NotFound) => bail!("Beatmap with id {map_id} was not found"),
            Err(MapError::Report(err)) => return Err(err),
        };

        let PpAttrs {
            calc,
            stars,
            max_combo,
            max_pp,
        } = PpAttrs::new(&map, self.mode, &self.mods, self.grade, self.pp).await;

        let pp = match self.pp {
            Some(pp) => pp,
            None => calc.score(&self).performance().await.pp() as f32,
        };

        let score = ScoreSlim {
            accuracy: self.accuracy,
            ended_at: self.ended_at,
            grade: self.grade,
            max_combo: self.max_combo,
            mode: self.mode,
            mods: self.mods,
            pp,
            score: self.score,
            classic_score: self.classic_score,
            score_id: self.score_id,
            legacy_id: self.legacy_id,
            statistics: self.statistics,
        };

        let global_idx_fut = async {
            if !matches!(
                map.status(),
                RankStatus::Ranked
                    | RankStatus::Loved
                    | RankStatus::Qualified
                    | RankStatus::Approved
            ) || score.grade == Grade::F
            {
                return None;
            }

            let map_lb_fut = Context::osu_scores().map_leaderboard(
                map_id,
                score.mode,
                None,
                50,
                self.legacy_scores,
            );

            let scores = match map_lb_fut.await {
                Ok(scores) => scores,
                Err(err) => {
                    warn!(?err, "Failed to get global scores");

                    return None;
                }
            };

            scores
                .iter()
                .position(|s| s.user_id == self.user_id && score.is_eq(s))
                .map(|idx| idx + 1)
        };

        let miss_analyzer_fut = async {
            let (score_id, guild_id) = score
                .legacy_id
                .zip(self.miss_analyzer_check.guild_id)
                .filter(|_| self.has_replay)?;

            debug!(score_id, "Sending score id to miss analyzer");

            match Context::client()
                .miss_analyzer_score_request(guild_id.get(), score_id)
                .await
            {
                Ok(wants_button) => wants_button.then_some(MissAnalyzerData { score_id }),
                Err(err) => {
                    warn!(?err, "Failed to send score id to miss analyzer");

                    None
                }
            }
        };

        let if_fc_fut = IfFc::new(&score, &map);

        let (global_idx, if_fc, miss_analyzer) =
            tokio::join!(global_idx_fut, if_fc_fut, miss_analyzer_fut);

        let if_fc_pp = if_fc.map(|if_fc| if_fc.pp);

        let replay = if self.with_render && self.has_replay {
            self.checksum
                .map(String::into_boxed_str)
                .and_then(|checksum| OwnedReplayScore::try_from_slim(&score, max_combo, checksum))
        } else {
            None
        };

        let pb_idx = self
            .top100
            .as_deref()
            .map(|top100| PersonalBestIndex::new(&score, map_id, map.status(), top100))
            .and_then(|pb_idx| ScoreEmbedDataPersonalBest::try_new(pb_idx, &self.origin));

        Ok(ScoreEmbedData {
            score,
            map,
            stars,
            max_combo,
            max_pp,
            replay,
            miss_analyzer,
            pb_idx,
            global_idx,
            if_fc_pp,
            #[cfg(feature = "twitch")]
            twitch: self.twitch,
        })
    }
}

struct PpAttrs<'m> {
    calc: PpManager<'m>,
    stars: f32,
    max_combo: u32,
    max_pp: f32,
}

impl<'m> PpAttrs<'m> {
    async fn new(
        map: &'m OsuMap,
        mode: GameMode,
        mods: &GameMods,
        grade: Grade,
        pp: Option<f32>,
    ) -> Self {
        let mut calc = Context::pp(map).mode(mode).mods(mods);
        let attrs = calc.performance().await;

        let max_pp = pp
            .filter(|_| grade.eq_letter(Grade::X) && mode != GameMode::Mania)
            .unwrap_or(attrs.pp() as f32);

        let stars = attrs.stars() as f32;
        let max_combo = attrs.max_combo();

        Self {
            calc,
            stars,
            max_combo,
            max_pp,
        }
    }
}

#[derive(Copy, Clone)]
pub struct MissAnalyzerCheck {
    guild_id: Option<Id<GuildMarker>>,
}

impl MissAnalyzerCheck {
    pub fn new(guild_id: Option<Id<GuildMarker>>, with_miss_analyzer: bool) -> Self {
        let guild_id = if with_miss_analyzer { guild_id } else { None };

        Self { guild_id }
    }

    pub fn without() -> Self {
        Self { guild_id: None }
    }
}

pub struct ScoreEmbedDataPersonalBest {
    /// Note that `idx` is 0-indexed.
    pub idx: Option<usize>,
    pub formatted: String,
}

pub struct MissAnalyzerData {
    pub score_id: u64,
}

impl ScoreEmbedDataPersonalBest {
    pub fn try_new(pb_idx: PersonalBestIndex, origin: &MessageOrigin) -> Option<Self> {
        let idx = match &pb_idx {
            PersonalBestIndex::FoundScore { idx } | PersonalBestIndex::Presumably { idx } => {
                Some(*idx)
            }
            PersonalBestIndex::FoundBetter { .. }
            | PersonalBestIndex::ScoreV1d { .. }
            | PersonalBestIndex::IfRanked { .. }
            | PersonalBestIndex::NotTop100 => None,
        };

        pb_idx
            .into_embed_description(origin)
            .map(|formatted| Self { idx, formatted })
    }

    /// Note that `idx` should be 0-indexed.
    pub fn from_index(idx: usize) -> Self {
        let origin = MessageOrigin::new(None, Id::new(1));

        let Some(formatted) = PersonalBestIndex::FoundScore { idx }.into_embed_description(&origin)
        else {
            unreachable!()
        };

        Self {
            idx: Some(idx),
            formatted,
        }
    }
}

impl<'q> Searchable<TopCriteria<'q>> for ScoreEmbedDataHalf {
    fn matches(&self, criteria: &FilterCriteria<TopCriteria<'q>>) -> bool {
        let mut matches = true;

        matches &= criteria.combo.contains(self.score.max_combo);
        matches &= criteria.miss.contains(self.score.statistics.count_miss);
        matches &= criteria.score.contains(self.score.score);
        matches &= criteria.date.contains(self.score.ended_at.date());
        matches &= criteria.stars.contains(self.stars);
        matches &= criteria.pp.contains(self.score.pp);
        matches &= criteria.acc.contains(self.score.accuracy);

        if !criteria.ranked_date.is_empty() {
            let Some(datetime) = self.map.ranked_date() else {
                return false;
            };
            matches &= criteria.ranked_date.contains(datetime.date());
        }

        let attrs = self.map.attributes().mods(self.score.mods.clone()).build();

        matches &= criteria.ar.contains(attrs.ar as f32);
        matches &= criteria.cs.contains(attrs.cs as f32);
        matches &= criteria.hp.contains(attrs.hp as f32);
        matches &= criteria.od.contains(attrs.od as f32);

        let keys = [
            (GameModIntermode::OneKey, 1.0),
            (GameModIntermode::TwoKeys, 2.0),
            (GameModIntermode::ThreeKeys, 3.0),
            (GameModIntermode::FourKeys, 4.0),
            (GameModIntermode::FiveKeys, 5.0),
            (GameModIntermode::SixKeys, 6.0),
            (GameModIntermode::SevenKeys, 7.0),
            (GameModIntermode::EightKeys, 8.0),
            (GameModIntermode::NineKeys, 9.0),
            (GameModIntermode::TenKeys, 10.0),
        ]
        .into_iter()
        .find_map(|(gamemod, keys)| self.score.mods.contains_intermode(gamemod).then_some(keys))
        .unwrap_or(attrs.cs as f32);

        matches &= self.map.mode() != GameMode::Mania || criteria.keys.contains(keys);

        if !matches
            || (criteria.length.is_empty()
                && criteria.bpm.is_empty()
                && criteria.artist.is_empty()
                && criteria.creator.is_empty()
                && criteria.version.is_empty()
                && criteria.title.is_empty()
                && !criteria.has_search_terms())
        {
            return matches;
        }

        let clock_rate = attrs.clock_rate as f32;
        matches &= criteria
            .length
            .contains(self.map.seconds_drain() as f32 / clock_rate);
        matches &= criteria.bpm.contains(self.map.bpm() * clock_rate);

        if !matches
            || (criteria.artist.is_empty()
                && criteria.creator.is_empty()
                && criteria.title.is_empty()
                && criteria.version.is_empty()
                && !criteria.has_search_terms())
        {
            return matches;
        }

        let artist = self.map.artist().cow_to_ascii_lowercase();
        matches &= criteria.artist.matches(&artist);

        let creator = self.map.creator().cow_to_ascii_lowercase();
        matches &= criteria.creator.matches(&creator);

        let version = self.map.version().cow_to_ascii_lowercase();
        matches &= criteria.version.matches(&version);

        let title = self.map.title().cow_to_ascii_lowercase();
        matches &= criteria.title.matches(&title);

        if matches && criteria.has_search_terms() {
            let terms = [artist, creator, version, title];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)))
        }

        matches
    }
}
