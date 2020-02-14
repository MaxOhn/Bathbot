use crate::{
    messages::{util, MapMultiData, AVATAR_URL, FLAG_URL, HOMEPAGE},
    util::{
        numbers::{round, round_and_comma, with_comma_u64},
        osu::{self, get_grade_emote},
        Error,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::cache::CacheRwLock;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

pub struct NoChokeData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub thumbnail: String,
    pub description: String,
}

impl NoChokeData {
    pub fn create(
        user: User,
        scores_data: HashMap<usize, (Score, Beatmap)>,
        cache: CacheRwLock,
    ) -> Result<MapMultiData, Error> {
        // 5 would be sufficient but 10 reduces error probability
        let mut index_10_pp: f32 = 0.0; // pp of 10th best unchoked score

        // BTreeMap to keep entries sorted by key
        let mut unchoked_scores: BTreeMap<F32T, (usize, Score)> = BTreeMap::new();
        for (idx, (score, map)) in scores_data.iter() {
            let combo_ratio = score.max_combo as f32 / map.max_combo.unwrap() as f32;
            // If the score is an (almost) fc but already has too few pp, skip
            if combo_ratio > 0.98 && score.pp.unwrap() < index_10_pp * 0.94 {
                continue;
            }
            let mut unchoked = score.clone();
            // If combo isn't max, unchoke the score
            if score.max_combo != map.max_combo.unwrap() {
                osu::unchoke_score(&mut unchoked, map)?;
            }
            let pp = unchoked.pp.unwrap();
            if pp > index_10_pp {
                unchoked_scores.insert(F32T::new(pp), (*idx, unchoked));
                index_10_pp = unchoked_scores
                    .iter()
                    .rev() // BTreeMap stores entries in ascending order wrt the key
                    .take(10)
                    .last() // Get 10th entry
                    .unwrap()
                    .0 // Get the entry's key
                    .to_f32(); // F32T to f32
            }
        }
        let unchoked_scores: Vec<(usize, Score, &Score, &Beatmap)> = unchoked_scores
            .into_iter()
            .rev()
            .take(5)
            .map(|(_, (i, unchoked_score))| {
                let (actual_score, map) = scores_data.get(&i).unwrap();
                (i, unchoked_score, actual_score, map)
            })
            .collect();

        // Done calculating, now preparing strings for message
        let author_icon = format!("{}{}.png", FLAG_URL, user.country);
        let author_url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);

        for (idx, unchoked, actual, map) in unchoked_scores.into_iter() {
            let (oppai, max_pp) = match osu::get_oppai(map.beatmap_id, actual, GameMode::STD) {
                Ok(tuple) => tuple,
                Err(why) => panic!("Something went wrong while using oppai: {}", why),
            };
            description.push_str(&format!(
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                 {grade} {old_pp} → **{new_pp}pp**/{max_pp}PP ~ ({old_acc} → **{new_acc}%**)\n\
                 [ {old_combo} → **{new_combo}x**/{max_combo}x ] ~ *Removed {misses} miss{plural}*\n",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = util::get_mods(&actual.enabled_mods),
                stars = util::get_stars(map, Some(oppai)),
                grade = get_grade_emote(unchoked.grade, cache.clone()),
                old_pp = round(actual.pp.unwrap()),
                new_pp = round(unchoked.pp.unwrap()),
                max_pp = round(max_pp),
                old_acc = round(actual.get_accuracy(GameMode::STD)),
                new_acc = round(unchoked.get_accuracy(GameMode::STD)),
                old_combo = actual.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.unwrap(),
                misses = actual.count_miss - unchoked.count_miss,
                plural = if actual.count_miss - unchoked.count_miss != 1 { "es" } else { "" }
            ));
        }
        // Remove the last '\n'
        description.pop();
        Ok(MapMultiData {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            description,
        })
    }
}

/// Providing a hashable, comparable alternative to f32 to put as key in a BTreeMap
#[derive(Hash, Eq, PartialEq)]
struct F32T {
    integral: u32,
    fractional: u32,
}

impl F32T {
    fn new(val: f32) -> Self {
        Self {
            integral: val.trunc() as u32,
            fractional: (val.fract() * 10_000.0) as u32,
        }
    }
}

impl F32T {
    fn to_f32(&self) -> f32 {
        self.integral as f32 + self.fractional as f32 / 10_000.0
    }
}

impl Ord for F32T {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.integral.cmp(&other.integral) {
            Ordering::Equal => self.fractional.cmp(&other.fractional),
            order => order,
        }
    }
}

impl PartialOrd for F32T {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
