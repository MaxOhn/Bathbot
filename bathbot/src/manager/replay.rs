use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult},
};

use bathbot_cache::Cache as BathbotCache;
use bathbot_client::Client as BathbotClient;
use bathbot_model::Either;
use bathbot_psql::{model::render::DbRenderOptions, Database};
use eyre::{Result, WrapErr};
use rosu_render::model::{RenderOptions, RenderResolution, RenderSkinOption, Skin, SkinInfo};
use rosu_v2::{
    model::score::LegacyScoreStatistics,
    prelude::{GameMode, Score, ScoreStatistics},
};
use time::{Date, OffsetDateTime, PrimitiveDateTime, Time};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{commands::osu::TopEntry, core::BotConfig};

#[derive(Copy, Clone)]
pub struct ReplayManager<'d> {
    psql: &'d Database,
    client: &'d BathbotClient,
    cache: &'d BathbotCache,
}

impl<'d> ReplayManager<'d> {
    pub fn new(psql: &'d Database, client: &'d BathbotClient, cache: &'d BathbotCache) -> Self {
        Self {
            psql,
            client,
            cache,
        }
    }

    pub async fn get_replay(
        self,
        score_id: Option<u64>,
        score: &ReplayScore<'_>,
    ) -> Result<Option<Box<[u8]>>> {
        let Some(score_id) = score_id else {
            return Ok(None);
        };

        match self.psql.select_osu_replay(score_id).await {
            Ok(Some(replay)) => return Ok(Some(replay)),
            Ok(None) => {}
            Err(err) => warn!(?err, "Failed to get replay from DB"),
        }

        // If the replay of a score id was not in the database, yet we requested it
        // already, that means the score has no available replay.
        let not_contained = self
            .cache
            .insert_into_set("__requested_replay_score_ids", score_id)
            .await
            .wrap_err("Failed to check whether replay was already requested")?;

        if !not_contained {
            return Ok(None);
        }

        let key = BotConfig::get().tokens.osu_key.as_ref();

        let raw_replay_opt = self
            .client
            .get_raw_osu_replay(key, score_id)
            .await
            .wrap_err("Failed to request replay")?;

        let Some(raw_replay) = raw_replay_opt else {
            return Ok(None);
        };

        let replay = complete_replay(score, score_id, &raw_replay);

        if let Err(err) = self.psql.insert_osu_replay(score_id, &replay).await {
            warn!(?err, "Failed to insert replay into DB");
        }

        Ok(Some(replay))
    }

    pub async fn get_settings(self, user: Id<UserMarker>) -> Result<ReplaySettings> {
        let options = self
            .psql
            .select_user_render_settings(user)
            .await
            .wrap_err("Failed to load settings")?;

        match options {
            Some(options) => Ok(ReplaySettings::from(options)),
            None => {
                let settings = ReplaySettings::default();

                if let Err(err) = self.set_settings(user, &settings).await {
                    warn!(?err);
                }

                Ok(settings)
            }
        }
    }

    pub async fn set_settings<'a>(
        self,
        user: Id<UserMarker>,
        settings: &ReplaySettings,
    ) -> Result<()> {
        let db_options = DbRenderOptions::from(settings);

        self.psql
            .upsert_user_render_settings(user, &db_options)
            .await
            .wrap_err("Failed to upsert settings")
    }

    pub async fn get_video_url(&self, score_id: u64) -> Result<Option<Box<str>>> {
        self.psql
            .select_replay_video_url(score_id)
            .await
            .wrap_err("Failed to get replay video url")
    }

    pub async fn store_video_url(&self, score_id: u64, video_url: &str) -> Result<()> {
        self.psql
            .upsert_replay_video_url(score_id, video_url)
            .await
            .wrap_err("Failed to store replay video url")
    }
}

#[derive(Default)]
pub struct ReplaySettings {
    options: RenderOptions,
    official_skin: ReplaySkin,
    custom_skin: Option<ReplaySkin>,
}

pub struct ReplaySkin {
    pub skin: RenderSkinOption<'static>,
    pub display_name: Box<str>,
}

impl Default for ReplaySkin {
    fn default() -> Self {
        Self {
            skin: RenderSkinOption::Official {
                name: "default".into(),
            },
            display_name: "Danser default skin (Redd glass)".into(),
        }
    }
}

impl ReplaySettings {
    pub fn new_with_official_skin(options: RenderOptions, skin: Skin) -> Self {
        Self {
            options,
            official_skin: ReplaySkin {
                skin: RenderSkinOption::from(skin.skin.into_string()),
                display_name: skin.presentation_name,
            },
            custom_skin: None,
        }
    }

    pub fn new_with_custom_skin(options: RenderOptions, skin: SkinInfo, id: u32) -> Self {
        Self {
            options,
            official_skin: ReplaySkin::default(),
            custom_skin: Some(ReplaySkin {
                skin: RenderSkinOption::Custom { id },
                display_name: skin.name,
            }),
        }
    }

    pub fn options(&self) -> &RenderOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut RenderOptions {
        &mut self.options
    }

    pub fn skin(&self, allow_custom_skin: bool) -> &ReplaySkin {
        if allow_custom_skin {
            self.custom_skin.as_ref().unwrap_or(&self.official_skin)
        } else {
            &self.official_skin
        }
    }

    pub fn official_skin(&mut self, skin: Skin) {
        self.official_skin = ReplaySkin {
            skin: RenderSkinOption::Official {
                name: skin.skin.into_string().into(),
            },
            display_name: skin.presentation_name,
        };
    }

    pub fn custom_skin(&mut self, id: u32, skin: SkinInfo) {
        self.custom_skin = Some(ReplaySkin {
            skin: RenderSkinOption::Custom { id },
            display_name: skin.name,
        });
    }

    pub fn remove_custom_skin(&mut self) {
        self.custom_skin.take();
    }

    pub fn skin_name(&self) -> (&str, Option<CustomSkinName<'_>>) {
        let custom = self.custom_skin.as_ref().map(|skin| {
            let RenderSkinOption::Custom { id } = skin.skin else {
                unreachable!()
            };

            CustomSkinName {
                name: skin.display_name.as_ref(),
                id,
            }
        });

        (self.official_skin.display_name.as_ref(), custom)
    }
}

pub struct CustomSkinName<'n> {
    name: &'n str,
    id: u32,
}

impl Display for CustomSkinName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} (ID {})", self.name, self.id)
    }
}

impl From<DbRenderOptions> for ReplaySettings {
    fn from(options: DbRenderOptions) -> Self {
        let settings = RenderOptions {
            resolution: RenderResolution::HD720,
            global_volume: options.global_volume as u8,
            music_volume: options.music_volume as u8,
            hitsound_volume: options.hitsound_volume as u8,
            show_hit_error_meter: options.show_hit_error_meter,
            show_unstable_rate: options.show_unstable_rate,
            show_score: options.show_score,
            show_hp_bar: options.show_hp_bar,
            show_combo_counter: options.show_combo_counter,
            show_pp_counter: options.show_pp_counter,
            show_key_overlay: options.show_key_overlay,
            show_scoreboard: options.show_scoreboard,
            show_borders: options.show_borders,
            show_mods: options.show_mods,
            show_result_screen: options.show_result_screen,
            use_skin_cursor: options.use_skin_cursor,
            use_skin_colors: options.use_skin_colors,
            use_skin_hitsounds: options.use_skin_hitsounds,
            use_beatmap_colors: options.use_beatmap_colors,
            cursor_scale_to_cs: options.cursor_scale_to_cs,
            cursor_rainbow: options.cursor_rainbow,
            cursor_trail_glow: options.cursor_trail_glow,
            draw_follow_points: options.draw_follow_points,
            draw_combo_numbers: options.draw_combo_numbers,
            cursor_size: options.cursor_size,
            cursor_trail: options.cursor_trail,
            beat_scaling: options.beat_scaling,
            slider_merge: options.slider_merge,
            objects_rainbow: options.objects_rainbow,
            flash_objects: options.flash_objects,
            use_slider_hitcircle_color: options.use_slider_hitcircle_color,
            seizure_warning: options.seizure_warning,
            load_storyboard: options.load_storyboard,
            load_video: options.load_video,
            intro_bg_dim: options.intro_bg_dim as u8,
            ingame_bg_dim: options.ingame_bg_dim as u8,
            break_bg_dim: options.break_bg_dim as u8,
            bg_parallax: options.bg_parallax,
            show_danser_logo: options.show_danser_logo,
            skip_intro: options.skip_intro,
            cursor_ripples: options.cursor_ripples,
            slider_snaking_in: options.slider_snaking_in,
            slider_snaking_out: options.slider_snaking_out,
            show_hit_counter: options.show_hit_counter,
            show_avatars_on_scoreboard: options.show_avatars_on_scoreboard,
            show_aim_error_meter: options.show_aim_error_meter,
            play_nightcore_samples: options.play_nightcore_samples,
            show_strain_graph: options.show_strain_graph,
            show_slider_breaks: options.show_slider_breaks,
            ignore_fail: options.ignore_fail,
        };

        let official_skin = ReplaySkin {
            skin: RenderSkinOption::Official {
                name: options.official_skin_name.into(),
            },
            display_name: options.official_skin_display_name.into(),
        };

        let custom_skin = options
            .custom_skin_id
            .zip(options.custom_skin_display_name)
            .map(|(id, name)| ReplaySkin {
                skin: RenderSkinOption::Custom { id: id as u32 },
                display_name: name.into(),
            });

        Self {
            options: settings,
            official_skin,
            custom_skin,
        }
    }
}

impl From<&ReplaySettings> for DbRenderOptions {
    fn from(settings: &ReplaySettings) -> Self {
        let ReplaySettings {
            options,
            official_skin,
            custom_skin,
        } = settings;

        let RenderSkinOption::Official { ref name } = official_skin.skin else {
            unreachable!()
        };

        let (custom_skin_id, custom_skin_display_name) = match custom_skin {
            Some(skin) => {
                let RenderSkinOption::Custom { id } = skin.skin else {
                    unreachable!()
                };
                let name = skin.display_name.as_ref().to_string();

                (Some(id as i32), Some(name))
            }
            None => (None, None),
        };

        Self {
            official_skin_name: name.as_ref().to_string(),
            official_skin_display_name: official_skin.display_name.as_ref().to_string(),
            custom_skin_id,
            custom_skin_display_name,
            global_volume: options.global_volume as i16,
            music_volume: options.music_volume as i16,
            hitsound_volume: options.hitsound_volume as i16,
            show_hit_error_meter: options.show_hit_error_meter,
            show_unstable_rate: options.show_unstable_rate,
            show_score: options.show_score,
            show_hp_bar: options.show_hp_bar,
            show_combo_counter: options.show_combo_counter,
            show_pp_counter: options.show_pp_counter,
            show_key_overlay: options.show_key_overlay,
            show_scoreboard: options.show_scoreboard,
            show_borders: options.show_borders,
            show_mods: options.show_mods,
            show_result_screen: options.show_result_screen,
            use_skin_cursor: options.use_skin_cursor,
            use_skin_colors: options.use_skin_colors,
            use_skin_hitsounds: options.use_skin_hitsounds,
            use_beatmap_colors: options.use_beatmap_colors,
            cursor_scale_to_cs: options.cursor_scale_to_cs,
            cursor_rainbow: options.cursor_rainbow,
            cursor_trail_glow: options.cursor_trail_glow,
            draw_follow_points: options.draw_follow_points,
            draw_combo_numbers: options.draw_combo_numbers,
            cursor_size: options.cursor_size,
            cursor_trail: options.cursor_trail,
            beat_scaling: options.beat_scaling,
            slider_merge: options.slider_merge,
            objects_rainbow: options.objects_rainbow,
            flash_objects: options.flash_objects,
            use_slider_hitcircle_color: options.use_slider_hitcircle_color,
            seizure_warning: options.seizure_warning,
            load_storyboard: options.load_storyboard,
            load_video: options.load_video,
            intro_bg_dim: options.intro_bg_dim as i16,
            ingame_bg_dim: options.ingame_bg_dim as i16,
            break_bg_dim: options.break_bg_dim as i16,
            bg_parallax: options.bg_parallax,
            show_danser_logo: options.show_danser_logo,
            skip_intro: options.skip_intro,
            cursor_ripples: options.cursor_ripples,
            slider_snaking_in: options.slider_snaking_in,
            slider_snaking_out: options.slider_snaking_out,
            show_hit_counter: options.show_hit_counter,
            show_avatars_on_scoreboard: options.show_avatars_on_scoreboard,
            show_aim_error_meter: options.show_aim_error_meter,
            play_nightcore_samples: options.play_nightcore_samples,
            show_strain_graph: options.show_strain_graph,
            show_slider_breaks: options.show_slider_breaks,
            ignore_fail: options.ignore_fail,
        }
    }
}

pub struct ReplayScore<'s> {
    // Hide inner enum so that the Borrowed variant cannot be constructed outside
    // and thus is certain to be validated when constructed through methods.
    inner: ReplayScoreInner<'s>,
}

impl From<OwnedReplayScore> for ReplayScore<'_> {
    fn from(score: OwnedReplayScore) -> Self {
        Self {
            inner: ReplayScoreInner::Owned(score),
        }
    }
}

enum ReplayScoreInner<'s> {
    Owned(OwnedReplayScore),
    Borrowed(&'s Score),
}

impl<'s> ReplayScore<'s> {
    /// Constructs a [`ReplayScore`], returning `None` if the `Score` had no map
    /// checksum.
    pub fn from_score(score: &'s Score) -> Option<Self> {
        score
            .map
            .as_ref()
            .is_some_and(|map| map.checksum.is_some())
            .then_some(ReplayScoreInner::Borrowed(score))
            .map(|inner| Self { inner })
    }

    fn mode(&self) -> GameMode {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.mode,
            ReplayScoreInner::Borrowed(score) => score.mode,
        }
    }

    fn ended_at(&self) -> OffsetDateTime {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.ended_at,
            ReplayScoreInner::Borrowed(score) => score.ended_at,
        }
    }

    fn map_checksum(&self) -> &str {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.map_checksum.as_ref(),
            ReplayScoreInner::Borrowed(score) => score
                .map
                .as_ref()
                .and_then(|map| map.checksum.as_deref())
                .expect("missing map checksum"),
        }
    }

    fn username(&self) -> &str {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.username.as_ref(),
            ReplayScoreInner::Borrowed(score) => score
                .user
                .as_ref()
                .map(|user| user.username.as_str())
                .unwrap_or_default(),
        }
    }

    fn statistics(&self) -> Either<&LegacyScoreStatistics, &ScoreStatistics> {
        match &self.inner {
            ReplayScoreInner::Owned(score) => Either::Left(&score.statistics),
            ReplayScoreInner::Borrowed(score) => Either::Right(&score.statistics),
        }
    }

    fn score(&self) -> u32 {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.score,
            ReplayScoreInner::Borrowed(score) => score.score,
        }
    }

    fn max_combo(&self) -> u16 {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.max_combo,
            ReplayScoreInner::Borrowed(score) => score.max_combo as u16,
        }
    }

    fn perfect(&self) -> bool {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.perfect,
            ReplayScoreInner::Borrowed(score) => score.legacy_perfect == Some(true),
        }
    }

    fn mods(&self) -> u32 {
        match &self.inner {
            ReplayScoreInner::Owned(score) => score.mods,
            ReplayScoreInner::Borrowed(score) => score.mods.bits(),
        }
    }
}

pub struct OwnedReplayScore {
    mode: GameMode,
    ended_at: OffsetDateTime,
    map_checksum: Box<str>,
    username: Box<str>,
    statistics: LegacyScoreStatistics,
    score: u32,
    max_combo: u16,
    perfect: bool,
    mods: u32,
}

impl OwnedReplayScore {
    pub fn from_top_entry(
        entry: &TopEntry,
        username: impl Into<Box<str>>,
        map_checksum: impl Into<Box<str>>,
    ) -> Self {
        Self {
            mode: entry.score.mode,
            ended_at: entry.score.ended_at,
            map_checksum: map_checksum.into(),
            username: username.into(),
            statistics: entry.score.statistics.clone(),
            score: entry.score.score,
            max_combo: entry.score.max_combo as u16,
            perfect: entry.max_combo == entry.score.max_combo,
            mods: entry.score.mods.bits(),
        }
    }

    pub fn from_score(score: &Score) -> Option<Self> {
        let map_checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref())?;

        Some(Self {
            mode: score.mode,
            ended_at: score.ended_at,
            map_checksum: Box::from(map_checksum),
            username: score
                .user
                .as_ref()
                .map(|user| user.username.as_str())
                .unwrap_or_default()
                .into(),
            statistics: score.statistics.as_legacy(score.mode),
            score: score.score,
            max_combo: score.max_combo as u16,
            perfect: score.legacy_perfect == Some(true),
            mods: score.mods.bits(),
        })
    }
}

// https://osu.ppy.sh/wiki/en/Client/File_formats/Osr_%28file_format%29
fn complete_replay(score: &ReplayScore<'_>, score_id: u64, raw_replay: &[u8]) -> Box<[u8]> {
    let mut replay = Vec::with_capacity(128 + raw_replay.len());

    let mut bytes_written = 0;

    bytes_written += encode_byte(&mut replay, score.mode() as u8);
    bytes_written += encode_int(&mut replay, game_version(score.ended_at().date()));

    let map_md5 = score.map_checksum();
    bytes_written += encode_string(&mut replay, map_md5);

    let username = score.username();
    bytes_written += encode_string(&mut replay, username);

    let replay_md5 = String::new();
    bytes_written += encode_string(&mut replay, &replay_md5);

    let stats = match score.statistics() {
        Either::Left(stats) => Cow::Borrowed(stats),
        Either::Right(stats) => Cow::Owned(stats.as_legacy(score.mode())),
    };

    bytes_written += encode_short(&mut replay, stats.count_300 as u16);
    bytes_written += encode_short(&mut replay, stats.count_100 as u16);
    bytes_written += encode_short(&mut replay, stats.count_50 as u16);
    bytes_written += encode_short(&mut replay, stats.count_geki as u16);
    bytes_written += encode_short(&mut replay, stats.count_katu as u16);
    bytes_written += encode_short(&mut replay, stats.count_miss as u16);

    bytes_written += encode_int(&mut replay, score.score());

    bytes_written += encode_short(&mut replay, score.max_combo());

    bytes_written += encode_byte(&mut replay, score.perfect() as u8);

    bytes_written += encode_int(&mut replay, score.mods());

    let lifebar = String::new();
    bytes_written += encode_string(&mut replay, &lifebar);

    bytes_written += encode_datetime(&mut replay, score.ended_at());

    bytes_written += encode_int(&mut replay, raw_replay.len() as u32);
    replay.extend_from_slice(raw_replay);

    bytes_written += encode_long(&mut replay, score_id);

    if bytes_written > 128 {
        warn!(bytes_written, "Wrote more bytes than initial allocation");
    }

    replay.into_boxed_slice()
}

fn encode_byte(bytes: &mut Vec<u8>, byte: u8) -> usize {
    bytes.push(byte);

    1
}

fn encode_short(bytes: &mut Vec<u8>, short: u16) -> usize {
    bytes.extend_from_slice(&short.to_le_bytes());

    2
}

fn encode_int(bytes: &mut Vec<u8>, int: u32) -> usize {
    bytes.extend_from_slice(&int.to_le_bytes());

    4
}

fn encode_long(bytes: &mut Vec<u8>, long: u64) -> usize {
    bytes.extend_from_slice(&long.to_le_bytes());

    8
}

fn encode_string(bytes: &mut Vec<u8>, s: &str) -> usize {
    if s.is_empty() {
        bytes.push(0x00); // "no string"

        1
    } else {
        bytes.push(0x0b); // "string incoming"
        let len = encode_leb128(bytes, s.len());
        bytes.extend_from_slice(s.as_bytes());

        1 + len + s.len()
    }
}

// https://en.wikipedia.org/wiki/LEB128
fn encode_leb128(bytes: &mut Vec<u8>, mut n: usize) -> usize {
    let mut bytes_written = 0;

    loop {
        let mut byte = ((n & u8::MAX as usize) as u8) & !(1 << 7);
        n >>= 7;

        if n != 0 {
            byte |= 1 << 7;
        }

        bytes.push(byte);
        bytes_written += 1;

        if n == 0 {
            return bytes_written;
        }
    }
}

// https://docs.microsoft.com/en-us/dotnet/api/system.datetime.ticks?redirectedfrom=MSDN&view=net-6.0#System_DateTime_Ticks
fn encode_datetime(bytes: &mut Vec<u8>, datetime: OffsetDateTime) -> usize {
    let orig_date = Date::from_ordinal_date(1, 1).unwrap();
    let orig_time = Time::from_hms(0, 0, 0).unwrap();

    let orig = PrimitiveDateTime::new(orig_date, orig_time).assume_utc();

    let orig_nanos = orig.unix_timestamp_nanos();
    let this_nanos = datetime.unix_timestamp_nanos();

    let long = (this_nanos - orig_nanos) / 100;

    encode_long(bytes, long as u64)
}

fn game_version(date: Date) -> u32 {
    let mut version = date.year() as u32;
    version *= 100;

    version += date.month() as u32;
    version *= 100;

    version += date.day() as u32;

    version
}
