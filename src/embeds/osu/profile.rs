use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        globals::AVATAR_URL,
        numbers::{round, with_comma_u64},
        osu::grade_emote,
    },
};

use itertools::Itertools;
use rosu::models::{Beatmap, GameMode, GameMods, Grade, Score, User};
use serenity::cache::Cache;
use std::{cmp::Ordering::Equal, collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct ProfileEmbed {
    description: Option<String>,
    author: Author,
    thumbnail: String,
    footer: Footer,
    fields: Vec<(String, String, bool)>,
}

impl ProfileEmbed {
    pub async fn new(
        user: User,
        score_maps: Vec<(Score, Beatmap)>,
        mode: GameMode,
        globals_count: HashMap<usize, usize>,
        cache: &Cache,
    ) -> Self {
        let footer_text = format!(
            "Joined osu! {} ({})",
            date_to_string(&user.join_date),
            how_long_ago(&user.join_date),
        );
        let bonus_pow = 0.9994_f64.powi(
            (user.count_ssh + user.count_ss + user.count_sh + user.count_s + user.count_a) as i32,
        );
        let bonus_pp = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
        let mut description = None;
        let mut fields = vec![
            (
                "Ranked score:".to_owned(),
                with_comma_u64(user.ranked_score),
                true,
            ),
            (
                "Total score:".to_owned(),
                with_comma_u64(user.total_score),
                true,
            ),
            (
                "Total hits:".to_owned(),
                with_comma_u64(user.total_hits()),
                true,
            ),
            (
                "Play count / time:".to_owned(),
                format!(
                    "{} / {} hrs",
                    with_comma_u64(user.playcount as u64),
                    user.total_seconds_played / 3600
                ),
                true,
            ),
            ("Level:".to_owned(), round(user.level).to_string(), true),
            ("Bonus PP:".to_owned(), format!("{}pp", bonus_pp), true),
            (
                "Accuracy:".to_owned(),
                format!("{}%", round(user.accuracy)),
                true,
            ),
        ];
        if score_maps.is_empty() {
            description = Some("No Top scores".to_string());
        } else {
            let values = ProfileResult::calc(mode, score_maps);
            let mut combo = String::from(&values.avg_combo.to_string());
            match mode {
                GameMode::STD | GameMode::CTB => {
                    let _ = write!(combo, "/{}", values.map_combo);
                }
                _ => {}
            }
            let _ = write!(combo, " [{} - {}]", values.min_combo, values.max_combo);
            fields.extend(vec![
                (
                    "Unweighted accuracy:".to_owned(),
                    format!(
                        "{}% [{}% - {}%]",
                        round(values.avg_acc),
                        round(values.min_acc),
                        round(values.max_acc)
                    ),
                    true,
                ),
                (
                    "Grades:".to_owned(),
                    format!(
                        "{}{} {}{} {}{} {}{} {}{}",
                        grade_emote(Grade::XH, cache).await,
                        user.count_ssh,
                        grade_emote(Grade::X, cache).await,
                        user.count_ss,
                        grade_emote(Grade::SH, cache).await,
                        user.count_sh,
                        grade_emote(Grade::S, cache).await,
                        user.count_s,
                        grade_emote(Grade::A, cache).await,
                        user.count_a,
                    ),
                    false,
                ),
                (
                    "Average PP:".to_owned(),
                    format!(
                        "{}pp [{} - {}]",
                        round(values.avg_pp),
                        round(values.min_pp),
                        round(values.max_pp)
                    ),
                    true,
                ),
                ("Average Combo:".to_owned(), combo, true),
            ]);
            if let Some(mod_combs_count) = values.mod_combs_count {
                fields.push((
                    "Favourite mod combinations:".to_owned(),
                    mod_combs_count
                        .into_iter()
                        .map(|(mods, count)| format!("`{} {}%`", mods, count))
                        .join(" > "),
                    false,
                ));
            }
            fields.reserve(if values.mod_combs_pp.is_some { 6 } else { 5 });
            fields.push((
                "Favourite mods:".to_owned(),
                values
                    .mods_count
                    .into_iter()
                    .map(|(mods, count)| format!("`{} {}%`", mods, count))
                    .join(" > "),
                false,
            ));
            if let Some(mod_combs_pp) = values.mod_combs_pp {
                fields.push((
                    "PP earned with mod combination:".to_owned(),
                    mod_combs_pp
                        .into_iter()
                        .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                        .join(" > "),
                    false,
                ));
            }
            fields.push((
                "PP earned with mod:".to_owned(),
                values
                    .mods_pp
                    .into_iter()
                    .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                    .join(" > "),
                false,
            ));
            fields.push((
                "Mappers in top 100:".to_owned(),
                values
                    .mappers
                    .into_iter()
                    .map(|(name, count, pp)| format!("{}: {}pp ({})", name, round(pp), count))
                    .join("\n"),
                true,
            ));
            let mut count_str = String::with_capacity(64);
            count_str.push_str("```\n");
            for (rank, count) in globals_count {
                let _ writeln!(count_str, "Top {:<2}: {}", rank, with_comma_u64(count as u64));
            }
            count_str.push_str("```");
            fields.push(("Global leaderboard count".to_owned(), count_str, true));
            fields.push((
                "Average map length:".to_owned(),
                format!(
                    "{} [{} - {}]",
                    sec_to_minsec(values.avg_len),
                    sec_to_minsec(values.min_len),
                    sec_to_minsec(values.max_len)
                ),
                true,
            ));
        }
        Self {
            description,
            fields,
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            footer: Footer::new(footer_text),
            author: osu::get_user_author(&user),
        }
    }
}

impl EmbedData for ProfileEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}

struct ProfileResult {
    min_acc: f32,
    max_acc: f32,
    avg_acc: f32,

    min_pp: f32,
    max_pp: f32,
    avg_pp: f32,

    min_combo: u32,
    max_combo: u32,
    avg_combo: u32,
    map_combo: u32,

    min_len: u32,
    max_len: u32,
    avg_len: u32,

    mappers: Vec<(String, u32, f32)>,

    mod_combs_count: Option<Vec<(GameMods, u32)>>,
    mod_combs_pp: Option<Vec<(GameMods, f32)>>,
    mods_count: Vec<(GameMods, u32)>,
    mods_pp: Vec<(GameMods, f32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, tuples: Vec<(Score, Beatmap)>) -> Self {
        let (mut min_acc, mut max_acc, mut avg_acc) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_pp, mut max_pp, mut avg_pp) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_combo, mut max_combo, mut avg_combo, mut map_combo) = (u32::MAX, 0, 0, 0);
        let (mut min_len, mut max_len, mut avg_len) = (f32::MAX, 0.0_f32, 0.0);
        let len = tuples.len() as f32;
        let mut mappers = HashMap::with_capacity(len as usize);
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut factor = 1.0;
        let mut mult_mods = false;
        for (score, map) in tuples {
            let acc = score.accuracy(mode);
            min_acc = min_acc.min(acc);
            max_acc = max_acc.max(acc);
            avg_acc += acc;

            if let Some(pp) = score.pp {
                min_pp = min_pp.min(pp);
                max_pp = max_pp.max(pp);
                avg_pp += pp;
            }

            min_combo = min_combo.min(score.max_combo);
            max_combo = max_combo.max(score.max_combo);
            avg_combo += score.max_combo;

            if let Some(combo) = map.max_combo {
                map_combo += combo;
            }

            let seconds_drain = if score.enabled_mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.enabled_mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 1.5
            } else {
                map.seconds_drain as f32
            };

            min_len = min_len.min(seconds_drain);
            max_len = max_len.max(seconds_drain);
            avg_len += seconds_drain;

            let mut mapper = mappers.entry(map.creator).or_insert((0, 0.0));
            let weighted_pp = score.pp.unwrap_or(0.0) * factor;
            factor *= 0.95;
            mapper.0 += 1;
            mapper.1 += weighted_pp;
            {
                let mut mod_comb = mod_combs.entry(score.enabled_mods).or_insert((0, 0.0));
                mod_comb.0 += 1;
                mod_comb.1 += weighted_pp;
            }
            if score.enabled_mods.is_empty() {
                let mut nm = mods.entry(GameMods::NoMod).or_insert((0, 0.0));
                nm.0 += 1;
                nm.1 += weighted_pp;
            } else {
                mult_mods |= score.enabled_mods.len() > 1;
                for m in score.enabled_mods {
                    let mut r#mod = mods.entry(m).or_insert((0, 0.0));
                    r#mod.0 += 1;
                    r#mod.1 += weighted_pp;
                }
            }
        }
        avg_acc /= len;
        avg_pp /= len;
        avg_combo /= len as u32;
        avg_len /= len;
        map_combo /= len as u32;
        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        mods.values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        let mut mappers: Vec<_> = mappers
            .into_iter()
            .map(|(name, (count, pp))| (name, count, pp))
            .collect();
        mappers.sort_by(
            |(_, count_a, pp_a), (_, count_b, pp_b)| match count_b.cmp(&count_a) {
                Equal => pp_b.partial_cmp(pp_a).unwrap_or(Equal),
                other => other,
            },
        );
        mappers = mappers[..5.min(mappers.len())].to_vec();
        let (mod_combs_count, mod_combs_pp) = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (*name, *count))
                .collect();
            mod_combs_count.sort_by(|a, b| b.1.cmp(&a.1));
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();
            mod_combs_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
            (Some(mod_combs_count), Some(mod_combs_pp))
        } else {
            (None, None)
        };
        let mut mods_count: Vec<_> = mods
            .iter()
            .map(|(name, (count, _))| (*name, *count))
            .collect();
        mods_count.sort_by(|a, b| b.1.cmp(&a.1));
        let mut mods_pp: Vec<_> = mods
            .into_iter()
            .map(|(name, (_, avg))| (name, avg))
            .collect();
        mods_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
        Self {
            min_acc,
            max_acc,
            avg_acc,
            min_pp,
            max_pp,
            avg_pp,
            min_combo,
            max_combo,
            avg_combo,
            map_combo,
            min_len: min_len as u32,
            max_len: max_len as u32,
            avg_len: avg_len as u32,
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
            mods_pp,
        }
    }
}
