use std::fmt::{Formatter, Result as FmtResult};

use rosu_mods::{GameMod, GameMode, GameMods};
use rosu_v2::model::Grade;
use serde::{Deserialize, Deserializer, de};
use time::OffsetDateTime;

use crate::deser::{adjust_acc, datetime_rfc3339, option_datetime_rfc3339};

fn deserialize_mods<'de, D: Deserializer<'de>>(d: D) -> Result<GameMods, D::Error> {
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = GameMods;

        fn expecting(&self, f: &mut Formatter) -> FmtResult {
            f.write_str("mods")
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut mods = GameMods::new();

            while let Some(s) = seq.next_element::<&'de str>()? {
                let (acronym, clock_rate) = s.split_once('x').unwrap_or((s, ""));

                let mut gamemod = GameMod::new(acronym, GameMode::Osu);

                match (!clock_rate.is_empty()).then(|| clock_rate.parse()) {
                    None => {}
                    Some(Ok(clock_rate)) => match gamemod {
                        GameMod::DoubleTimeOsu(ref mut m) => m.speed_change = Some(clock_rate),
                        GameMod::NightcoreOsu(ref mut m) => m.speed_change = Some(clock_rate),
                        GameMod::HalfTimeOsu(ref mut m) => m.speed_change = Some(clock_rate),
                        GameMod::DaycoreOsu(ref mut m) => m.speed_change = Some(clock_rate),
                        _ => {}
                    },
                    Some(Err(_)) => {
                        return Err(de::Error::custom(format!(
                            "expected clock rate; got `{clock_rate}`"
                        )));
                    }
                }

                mods.insert(gamemod);
            }

            Ok(mods)
        }
    }

    d.deserialize_seq(Visitor)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxScore {
    pub id: u64,
    pub user_id: u32,
    // Comes as a null from /api/players/{user_id}/scores
    pub user: Option<RelaxUser>,
    pub beatmap_id: u32,
    pub beatmap: RelaxBeatmap,
    pub grade: Grade,
    #[serde(with = "adjust_acc")]
    pub accuracy: f32,
    pub combo: u32,
    #[serde(deserialize_with = "deserialize_mods")]
    pub mods: GameMods,
    #[serde(with = "datetime_rfc3339")]
    pub date: OffsetDateTime,
    pub total_score: u32,
    pub count_50: u32,
    pub count_100: u32,
    pub count_300: u32,
    pub count_miss: u32,
    pub spinner_bonus: Option<u32>,
    pub spinner_spins: Option<u32>,
    pub legacy_slider_ends: Option<u32>,
    pub slider_ticks: Option<u32>,
    pub slider_ends: Option<u32>,
    pub legacy_slider_end_misses: Option<u32>,
    pub slider_tick_misses: Option<u32>,
    pub pp: Option<f64>,
    pub is_best: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxUser {
    pub id: u32,
    pub country_code: Option<String>,
    pub username: Option<String>,
    pub total_pp: Option<f64>,
    pub total_accuracy: Option<f64>,
    #[serde(with = "option_datetime_rfc3339")]
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxBeatmap {
    pub id: u32,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub creator_id: u32,
    pub beatmap_set_id: u32,
    pub difficulty_name: Option<String>,
    pub approach_rate: f64,
    pub overall_difficulty: f64,
    pub circle_size: f64,
    pub health_drain: f64,
    pub beats_per_minute: f64,
    pub circles: u32,
    pub sliders: u32,
    pub spinners: u32,
    pub star_rating_normal: f64,
    pub star_rating: Option<f64>,
    pub status: RelaxBeatmapStatus,
    pub max_combo: u32,
}

#[derive(Debug, Deserialize)]
pub enum RelaxBeatmapStatus {
    Graveyard,
    Wip,
    Pending,
    Ranked,
    Approved,
    Qualified,
    Loved,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxAllowedModsResponse {
    mods: Option<Vec<String>>,
    mod_settings: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxListingBeatmap {
    id: u32,
    artist: Option<String>,
    title: Option<String>,
    creator_id: u32,
    beatmap_set_id: u32,
    difficulty_name: Option<String>,
    star_rating: Option<f32>,
    status: RelaxBeatmapStatus,
    playcount: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlaycountPerMonth {
    #[serde(with = "datetime_rfc3339")]
    pub date: OffsetDateTime,
    pub playcount: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxRecentScoresResponse {
    scores: Option<Vec<RelaxScore>>,
    scores_today: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxStatsResponse {
    scores_total: u32,
    users_total: u32,
    beatmaps_total: u32,
    latest_score_id: u64,
    scores_in_a_month: u32,
    playcount_per_day: Option<RelaxPlaycountPerMonth>,
    playcount_per_month: Option<RelaxPlaycountPerMonth>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlayersDataResponse {
    pub id: u32,
    pub country_code: Option<String>,
    pub username: Option<String>,
    pub total_pp: Option<f64>,
    pub total_accuracy: Option<f64>,
    #[serde(with = "option_datetime_rfc3339")]
    pub updated_at: Option<OffsetDateTime>,
    pub rank: Option<u32>,
    pub country_rank: Option<u32>,
    pub playcount: u32,
    #[serde(rename = "countSS")]
    pub count_ss: u32,
    #[serde(rename = "countS")]
    pub count_s: u32,
    #[serde(rename = "countA")]
    pub count_a: u32,
    pub playcounts_per_month: Vec<RelaxPlaycountPerMonth>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxPlayersResult {
    players: Vec<Option<RelaxUser>>,
    total: u32,
}
