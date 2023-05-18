use std::{
    collections::{hash_map::Iter, HashMap},
    hash::BuildHasher,
};

use eyre::Result;
use rosu_v2::prelude::{GameMode, Grade, RankStatus, ScoreStatistics};
use time::OffsetDateTime;

use crate::Database;

type Maps<S> = HashMap<u32, DbScoreBeatmap, S>;
type Mapsets<S> = HashMap<u32, DbScoreBeatmapset, S>;
type Users<S> = HashMap<u32, DbScoreUser, S>;

#[derive(Default)]
pub struct DbScoresBuilder<'a> {
    pub(crate) mode: Option<GameMode>,
    pub(crate) country_code: Option<&'a str>,
    pub(crate) map_id: Option<i32>,
    pub(crate) grade: Option<Grade>,
    pub(crate) mods_include: Option<i32>,
    pub(crate) mods_exclude: Option<i32>,
    pub(crate) mods_exact: Option<i32>,
}

impl<'a> DbScoresBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn build_osu<S>(&self, psql: &Database, users: &[i32]) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        psql.select_scores_by_osu_id::<S>(users, self).await
    }

    pub async fn build_discord<S>(&self, psql: &Database, users: &[i64]) -> Result<DbScores<S>>
    where
        S: BuildHasher + Default,
    {
        psql.select_scores_by_discord_id::<S>(users, self).await
    }

    pub fn mode(&mut self, mode: GameMode) -> &mut Self {
        self.mode = Some(mode);

        self
    }

    pub fn country_code(&mut self, country_code: &'a str) -> &mut Self {
        self.country_code = Some(country_code);

        self
    }

    pub fn map_id(&mut self, map_id: i32) -> &mut Self {
        self.map_id = Some(map_id);

        self
    }

    pub fn grade(&mut self, grade: Grade) -> &mut Self {
        self.grade = Some(grade);

        self
    }

    pub fn mods_include(&mut self, mods: i32) -> &mut Self {
        self.mods_include = Some(mods);

        self
    }

    pub fn mods_exclude(&mut self, mods: i32) -> &mut Self {
        self.mods_exclude = Some(mods);

        self
    }

    pub fn mods_exact(&mut self, mods: i32) -> &mut Self {
        self.mods_exact = Some(mods);

        self
    }
}

pub struct DbScores<S> {
    pub(crate) scores: Vec<DbScore>,
    pub(crate) maps: Maps<S>,
    pub(crate) mapsets: Mapsets<S>,
    pub(crate) users: Users<S>,
}

impl<S> DbScores<S> {
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    pub fn scores(&self) -> &[DbScore] {
        &self.scores
    }

    pub fn scores_mut(&mut self) -> &mut [DbScore] {
        &mut self.scores
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&DbScore, &Maps<S>, &Mapsets<S>, &Users<S>) -> bool,
    {
        self.scores
            .retain(|score| f(score, &self.maps, &self.mapsets, &self.users));
    }

    pub fn maps(&self) -> Iter<'_, u32, DbScoreBeatmap> {
        self.maps.iter()
    }

    pub fn mapsets(&self) -> Iter<'_, u32, DbScoreBeatmapset> {
        self.mapsets.iter()
    }

    pub fn users(&self) -> Iter<'_, u32, DbScoreUser> {
        self.users.iter()
    }

    pub fn user_count(&self) -> usize {
        self.users.len()
    }
}

impl<S: BuildHasher> DbScores<S> {
    pub fn map(&self, map_id: u32) -> Option<&DbScoreBeatmap> {
        self.maps.get(&map_id)
    }

    pub fn mapset(&self, mapset_id: u32) -> Option<&DbScoreBeatmapset> {
        self.mapsets.get(&mapset_id)
    }

    pub fn user(&self, user_id: u32) -> Option<&DbScoreUser> {
        self.users.get(&user_id)
    }
}

pub struct DbScore {
    pub ended_at: OffsetDateTime,
    pub grade: Grade,
    pub map_id: u32,
    pub max_combo: u32,
    pub mode: GameMode,
    pub mods: u32,
    pub pp: Option<f32>,
    pub score: u32,
    pub score_id: u64,
    pub stars: Option<f32>,
    pub statistics: ScoreStatistics,
    pub user_id: u32,
}

pub struct DbScoreBeatmapRaw {
    pub map_id: i32,
    pub mapset_id: i32,
    pub user_id: i32,
    pub map_version: String,
    pub seconds_drain: i32,
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub ar: f32,
    pub bpm: f32,
}

pub struct DbScoreBeatmap {
    pub mapset_id: u32,
    pub creator_id: u32,
    pub version: Box<str>,
    pub seconds_drain: u32,
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub ar: f32,
    pub bpm: f32,
}

impl From<DbScoreBeatmapRaw> for DbScoreBeatmap {
    fn from(map: DbScoreBeatmapRaw) -> Self {
        Self {
            mapset_id: map.mapset_id as u32,
            creator_id: map.user_id as u32,
            version: map.map_version.into_boxed_str(),
            seconds_drain: map.seconds_drain as u32,
            hp: map.hp,
            cs: map.cs,
            od: map.od,
            ar: map.ar,
            bpm: map.bpm,
        }
    }
}

pub struct DbScoreBeatmapsetRaw {
    pub mapset_id: i32,
    pub artist: String,
    pub title: String,
    pub rank_status: i16,
    pub ranked_date: Option<OffsetDateTime>,
}

pub struct DbScoreBeatmapset {
    pub artist: Box<str>,
    pub title: Box<str>,
    pub rank_status: RankStatus,
    pub ranked_date: Option<OffsetDateTime>,
}

impl From<DbScoreBeatmapsetRaw> for DbScoreBeatmapset {
    fn from(mapset: DbScoreBeatmapsetRaw) -> Self {
        Self {
            artist: mapset.artist.into_boxed_str(),
            title: mapset.title.into_boxed_str(),
            rank_status: parse_status(mapset.rank_status),
            ranked_date: mapset.ranked_date,
        }
    }
}

pub struct DbScoreUserRaw {
    pub user_id: i32,
    pub username: String,
}

pub struct DbScoreUser {
    pub username: Box<str>,
}

impl From<DbScoreUserRaw> for DbScoreUser {
    fn from(user: DbScoreUserRaw) -> Self {
        Self {
            username: user.username.into_boxed_str(),
        }
    }
}

pub(crate) struct DbScoreAny {
    pub score_id: i64,
    pub user_id: i32,
    pub map_id: i32,
    pub gamemode: i16,
    pub mods: i32,
    pub score: i32,
    pub maxcombo: i32,
    pub grade: i16,
    pub count50: i32,
    pub count100: i32,
    pub count300: i32,
    pub countgeki: i32,
    pub countkatu: i32,
    pub countmiss: i32,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub stars_osu: Option<f32>,
    pub stars_taiko: Option<f32>,
    pub stars_catch: Option<f32>,
    pub stars_mania: Option<f32>,
}

pub(crate) struct DbScoreOsu {
    pub score_id: i64,
    pub user_id: i32,
    pub map_id: i32,
    pub mods: i32,
    pub score: i32,
    pub maxcombo: i32,
    pub grade: i16,
    pub count50: i32,
    pub count100: i32,
    pub count300: i32,
    pub countmiss: i32,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub stars: Option<f32>,
}

pub(crate) struct DbScoreTaiko {
    pub score_id: i64,
    pub user_id: i32,
    pub map_id: i32,
    pub mods: i32,
    pub score: i32,
    pub maxcombo: i32,
    pub grade: i16,
    pub count100: i32,
    pub count300: i32,
    pub countmiss: i32,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub stars: Option<f32>,
}

pub(crate) struct DbScoreCatch {
    pub score_id: i64,
    pub user_id: i32,
    pub map_id: i32,
    pub mods: i32,
    pub score: i32,
    pub maxcombo: i32,
    pub grade: i16,
    pub count50: i32,
    pub count100: i32,
    pub count300: i32,
    pub countmiss: i32,
    pub countkatu: i32,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub stars: Option<f32>,
}

pub(crate) struct DbScoreMania {
    pub score_id: i64,
    pub user_id: i32,
    pub map_id: i32,
    pub mods: i32,
    pub score: i32,
    pub maxcombo: i32,
    pub grade: i16,
    pub count50: i32,
    pub count100: i32,
    pub count300: i32,
    pub countmiss: i32,
    pub countgeki: i32,
    pub countkatu: i32,
    pub ended_at: OffsetDateTime,
    pub pp: Option<f32>,
    pub stars: Option<f32>,
}

fn parse_mode(mode: i16) -> GameMode {
    match mode {
        0 => GameMode::Osu,
        1 => GameMode::Taiko,
        2 => GameMode::Catch,
        3 => GameMode::Mania,
        _ => unreachable!(),
    }
}

fn parse_grade(grade: i16) -> Grade {
    match grade {
        0 => Grade::F,
        1 => Grade::D,
        2 => Grade::C,
        3 => Grade::B,
        4 => Grade::A,
        5 => Grade::S,
        6 => Grade::SH,
        7 => Grade::X,
        8 => Grade::XH,
        _ => unreachable!(),
    }
}

fn parse_status(status: i16) -> RankStatus {
    match status {
        -2 => RankStatus::Graveyard,
        -1 => RankStatus::WIP,
        0 => RankStatus::Pending,
        1 => RankStatus::Ranked,
        2 => RankStatus::Approved,
        3 => RankStatus::Qualified,
        4 => RankStatus::Loved,
        _ => unreachable!(),
    }
}

impl From<DbScoreAny> for DbScore {
    fn from(score: DbScoreAny) -> Self {
        let mode = parse_mode(score.gamemode);

        Self {
            ended_at: score.ended_at,
            grade: parse_grade(score.grade),
            map_id: score.map_id as u32,
            max_combo: score.maxcombo as u32,
            mode,
            mods: score.mods as u32,
            pp: score.pp,
            score: score.score as u32,
            score_id: score.score_id as u64,
            stars: match mode {
                GameMode::Osu => score.stars_osu,
                GameMode::Taiko => score.stars_taiko,
                GameMode::Catch => score.stars_catch,
                GameMode::Mania => score.stars_mania,
            },
            statistics: ScoreStatistics {
                count_geki: score.countgeki as u32,
                count_300: score.count300 as u32,
                count_katu: score.countkatu as u32,
                count_100: score.count100 as u32,
                count_50: score.count50 as u32,
                count_miss: score.countmiss as u32,
            },
            user_id: score.user_id as u32,
        }
    }
}

impl From<(DbScoreOsu, GameMode)> for DbScore {
    fn from((score, mode): (DbScoreOsu, GameMode)) -> Self {
        Self {
            ended_at: score.ended_at,
            grade: parse_grade(score.grade),
            map_id: score.map_id as u32,
            max_combo: score.maxcombo as u32,
            mode,
            mods: score.mods as u32,
            pp: score.pp,
            score: score.score as u32,
            score_id: score.score_id as u64,
            stars: score.stars,
            statistics: ScoreStatistics {
                count_geki: 0,
                count_300: score.count300 as u32,
                count_katu: 0,
                count_100: score.count100 as u32,
                count_50: score.count50 as u32,
                count_miss: score.countmiss as u32,
            },
            user_id: score.user_id as u32,
        }
    }
}

impl From<(DbScoreTaiko, GameMode)> for DbScore {
    fn from((score, mode): (DbScoreTaiko, GameMode)) -> Self {
        Self {
            ended_at: score.ended_at,
            grade: parse_grade(score.grade),
            map_id: score.map_id as u32,
            max_combo: score.maxcombo as u32,
            mode,
            mods: score.mods as u32,
            pp: score.pp,
            score: score.score as u32,
            score_id: score.score_id as u64,
            stars: score.stars,
            statistics: ScoreStatistics {
                count_geki: 0,
                count_300: score.count300 as u32,
                count_katu: 0,
                count_100: score.count100 as u32,
                count_50: 0,
                count_miss: score.countmiss as u32,
            },
            user_id: score.user_id as u32,
        }
    }
}

impl From<(DbScoreCatch, GameMode)> for DbScore {
    fn from((score, mode): (DbScoreCatch, GameMode)) -> Self {
        Self {
            ended_at: score.ended_at,
            grade: parse_grade(score.grade),
            map_id: score.map_id as u32,
            max_combo: score.maxcombo as u32,
            mode,
            mods: score.mods as u32,
            pp: score.pp,
            score: score.score as u32,
            score_id: score.score_id as u64,
            stars: score.stars,
            statistics: ScoreStatistics {
                count_geki: 0,
                count_300: score.count300 as u32,
                count_katu: score.countkatu as u32,
                count_100: score.count100 as u32,
                count_50: score.count50 as u32,
                count_miss: score.countmiss as u32,
            },
            user_id: score.user_id as u32,
        }
    }
}

impl From<(DbScoreMania, GameMode)> for DbScore {
    fn from((score, mode): (DbScoreMania, GameMode)) -> Self {
        Self {
            ended_at: score.ended_at,
            grade: parse_grade(score.grade),
            map_id: score.map_id as u32,
            max_combo: score.maxcombo as u32,
            mode,
            mods: score.mods as u32,
            pp: score.pp,
            score: score.score as u32,
            score_id: score.score_id as u64,
            stars: score.stars,
            statistics: ScoreStatistics {
                count_geki: score.countgeki as u32,
                count_300: score.count300 as u32,
                count_katu: score.countkatu as u32,
                count_100: score.count100 as u32,
                count_50: score.count50 as u32,
                count_miss: score.countmiss as u32,
            },
            user_id: score.user_id as u32,
        }
    }
}
