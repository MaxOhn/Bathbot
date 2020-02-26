use crate::{
    messages::util,
    util::{
        datetime::how_long_ago,
        globals::{AVATAR_URL, HOMEPAGE},
        numbers::{round, round_and_comma, round_precision, with_comma_u64},
        osu,
        pp::PPProvider,
        Error,
    },
};

use rosu::models::{Beatmap, GameMode, Grade, Score, User};
use serenity::{cache::CacheRwLock, prelude::Context};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

pub struct AuthorDescThumbData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub thumbnail: String,
    pub description: String,
}

impl AuthorDescThumbData {
    fn get_user_author(user: &User) -> (String, String, String) {
        let icon = format!("{}/images/flags/{}.png", HOMEPAGE, user.country);
        let url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        (icon, url, text)
    }

    pub fn create_top(
        user: User,
        scores_data: Vec<(usize, Score, Beatmap)>,
        mode: GameMode,
        ctx: &Context,
    ) -> Result<Self, Error> {
        let (author_icon, author_url, author_text) = Self::get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);
        for (idx, score, map) in scores_data.iter() {
            let pp_provider = match PPProvider::new(score, map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
            description.push_str(&format!(
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                 {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}\n",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = util::get_mods(&score.enabled_mods),
                stars = util::get_stars(&map, pp_provider.oppai()),
                grade = osu::grade_emote(score.grade, ctx.cache.clone()),
                pp = util::get_pp(score, &pp_provider, mode),
                acc = util::get_acc(&score, mode),
                score = with_comma_u64(score.score as u64),
                combo = util::get_combo(&score, &map),
                hits = util::get_hits(&score, mode),
                ago = how_long_ago(&score.date),
            ));
        }
        description.pop();
        Ok(Self {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            description,
        })
    }

    pub fn create_ratio(user: User, scores: Vec<Score>) -> Result<Self, Error> {
        let accs = [0, 90, 95, 97, 99];
        let mut categories: BTreeMap<u8, RatioCategory> = BTreeMap::new();
        for &acc in accs.iter() {
            categories.insert(acc, RatioCategory::default());
        }
        categories.insert(100, RatioCategory::default());
        for score in scores {
            let acc = score.accuracy(GameMode::MNA);
            for &curr in accs.iter() {
                if acc > curr as f32 {
                    categories.get_mut(&curr).unwrap().add_score(&score);
                }
            }
            if score.grade.eq_letter(Grade::X) {
                categories.get_mut(&100).unwrap().add_score(&score);
            }
        }
        let (author_icon, author_url, author_text) = Self::get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(256);
        description.push_str(
            "```\n \
             Acc: #Scores | Ratio | % misses\n\
             --------------+-------+---------\n",
        );
        for (acc, c) in categories.into_iter() {
            if c.scores > 0 {
                description.push_str(&format!(
                    "{}{:>2}%: {:>7} | {:>5} | {:>7}%\n",
                    if acc < 100 { ">" } else { "" },
                    acc,
                    c.scores,
                    c.get_ratio(),
                    c.get_miss_percent(),
                ));
            }
        }
        description.push_str("```");
        Ok(Self {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            description,
        })
    }

    pub fn create_nochoke(
        user: User,
        scores_data: HashMap<usize, (Score, Beatmap)>,
        cache: CacheRwLock,
    ) -> Result<Self, Error> {
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
        let (author_icon, author_url, author_text) = Self::get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);

        for (idx, unchoked, actual, map) in unchoked_scores.into_iter() {
            let pp_provider = match PPProvider::new(actual, map, None) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
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
                stars = util::get_stars(map, pp_provider.oppai()),
                grade = osu::grade_emote(unchoked.grade, cache.clone()),
                old_pp = round(actual.pp.unwrap()),
                new_pp = round(unchoked.pp.unwrap()),
                max_pp = round(pp_provider.max_pp()),
                old_acc = round(actual.accuracy(GameMode::STD)),
                new_acc = round(unchoked.accuracy(GameMode::STD)),
                old_combo = actual.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.unwrap(),
                misses = actual.count_miss - unchoked.count_miss,
                plural = if actual.count_miss - unchoked.count_miss != 1 { "es" } else { "" }
            ));
        }
        // Remove the last '\n'
        description.pop();
        Ok(Self {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            description,
        })
    }
}

#[derive(Default)]
struct RatioCategory {
    pub scores: u8,
    pub count_geki: u32,
    pub count_300: u32,
    pub count_miss: u32,
    pub count_objects: u32,
}

impl RatioCategory {
    fn add_score(&mut self, s: &Score) {
        self.scores += 1;
        self.count_geki += s.count_geki;
        self.count_300 += s.count300;
        self.count_miss += s.count_miss;
        self.count_objects +=
            s.count_geki + s.count300 + s.count_katu + s.count100 + s.count50 + s.count_miss;
    }

    fn get_ratio(&self) -> f32 {
        if self.count_300 == 0 {
            if self.count_geki > 0 {
                1.0
            } else {
                0.0
            }
        } else {
            round_precision(self.count_geki as f32 / self.count_300 as f32, 3)
        }
    }

    fn get_miss_percent(&self) -> f32 {
        if self.count_objects > 0 {
            round_precision(
                100.0 * self.count_miss as f32 / self.count_objects as f32,
                3,
            )
        } else {
            0.0
        }
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
