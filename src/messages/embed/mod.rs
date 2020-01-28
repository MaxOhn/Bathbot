#![allow(clippy::too_many_arguments)]
#![allow(unused)]

mod util;

use crate::util::{
    datetime::{date_to_string, how_long_ago, sec_to_minsec},
    numbers::{round, round_and_comma, with_comma_u64},
    osu::*,
};

use itertools::Itertools;
use roppai::Oppai;
use rosu::models::{Beatmap, GameMod, GameMode, GameMods, Score, User};
use serenity::{builder::CreateEmbed, cache::CacheRwLock, utils::Colour};
use std::{cmp::Ordering::Equal, collections::HashMap, f32, u32};

const HOMEPAGE: &str = "https://osu.ppy.sh/";
const MAP_THUMB_URL: &str = "https://b.ppy.sh/thumb/";
const AVATAR_URL: &str = "https://a.ppy.sh/";
const FLAG_URL: &str = "https://osu.ppy.sh//images/flags/";

pub struct BotEmbed {
    cache: CacheRwLock,
    mode: GameMode,
    embed: EmbedType,
}

impl BotEmbed {
    pub fn new(cache: CacheRwLock, mode: GameMode, embed_type: EmbedType) -> Self {
        Self {
            cache,
            mode,
            embed: embed_type,
        }
    }

    pub fn create(self, e: &mut CreateEmbed) -> &mut CreateEmbed {
        self.embed.create(e, self.mode, self.cache)
    }
}

pub enum EmbedType {
    // user - score - map - user top scores - map global leaderboard
    UserScoreSingle(Box<User>, Box<Score>, Box<Beatmap>, Vec<Score>, Vec<Score>),
    // user - map - scores of user on map
    UserScoreMulti(Box<User>, Box<Beatmap>, Vec<Score>),
    // user - (score-map) - score indices
    UserMapMulti(Box<User>, Vec<(Score, Beatmap)>, Option<Vec<usize>>),
    // user - (score-map)
    Profile(Box<User>, Vec<(Score, Beatmap)>),
    // score - map
    SimulateScore(Option<Box<Score>>, Box<Beatmap>),
    // map - (user-score)
    UserLeaderboard(Box<Beatmap>, Vec<(User, Score)>),
    // user - user top 100
    ManiaRatio(Box<User>, Vec<Score>),
    // compared users - common maps (assumed to be in desired order)
    UserCommonScores(Vec<User>, Vec<Beatmap>),
}

impl EmbedType {
    fn create(self, e: &mut CreateEmbed, mode: GameMode, cache: CacheRwLock) -> &mut CreateEmbed {
        e.color(Colour::DARK_GREEN);
        use EmbedType::*;
        match self {
            UserScoreSingle(user, score, map, personal, global) => {
                create_user_score_single(e, user, mode, score, map, personal, global, cache)
            }
            UserScoreMulti(user, map, scores) => {
                create_user_score_multi(e, user, mode, map, scores, cache)
            }
            UserMapMulti(user, tuples, indices) => {
                let indices: Vec<usize> = indices.unwrap_or_else(|| (1..=tuples.len()).collect());
                create_user_map_multi(e, user, mode, tuples, indices, cache)
            }
            Profile(user, tuples) => create_profile(e, user, mode, tuples, cache),
            SimulateScore(score, map) => create_simulation(e, mode, score, map, cache),
            UserLeaderboard(map, tuples) => create_leaderboard(e, mode, map, tuples, cache),
            ManiaRatio(user, scores) => create_ratio(e, user, scores),
            UserCommonScores(users, maps) => create_common(e, users, maps),
        }
    }
}

fn create_user_score_single(
    embed: &mut CreateEmbed,
    user: Box<User>,
    mode: GameMode,
    score: Box<Score>,
    map: Box<Beatmap>,
    personal: Vec<Score>,
    global: Vec<Score>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    // Set description with index in personal / global top scores
    let personal_idx = personal.into_iter().position(|s| s == *score);
    let global_idx = global.into_iter().position(|s| s == *score);
    if personal_idx.is_some() || global_idx.is_some() {
        let mut description = String::from("__**");
        if let Some(idx) = personal_idx {
            description.push_str("Personal Best #");
            description.push_str(&(idx + 1).to_string());
            if global_idx.is_some() {
                description.push_str(" and ");
            }
        }
        if let Some(idx) = global_idx {
            description.push_str("Global Top #");
            description.push_str(&(idx + 1).to_string());
        }
        description.push_str("**__");
        embed.description(description);
    }

    // Set title with (mania keys, ) artist, title, and version
    let title = if mode == GameMode::MNA {
        format!("{} {}", util::get_keys(&score.enabled_mods, &*map), map)
    } else {
        map.to_string()
    };
    embed.title(title);

    // TODO: Handle GameMode's differently
    let (oppai, max_pp) = match get_oppai(map.beatmap_id, &score, mode) {
        Ok(tuple) => tuple,
        Err(why) => panic!("Something went wrong while using oppai: {}", why),
    };
    let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
    embed
        .url(format!("{}b/{}", HOMEPAGE, map.beatmap_id))
        .timestamp(date_to_string(&score.date))
        .thumbnail(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
        .footer(|f| {
            f.icon_url(format!("{}{}", AVATAR_URL, map.creator_id))
                .text(format!("{:?} map by {}", map.approval_status, map.creator))
        })
        .fields(vec![
            (
                "Grade",
                util::get_grade_completion_mods(&score, mode, &map, cache),
                true,
            ),
            ("Score", with_comma_u64(score.score as u64), true),
            ("Acc", util::get_acc(&score, mode), true),
            ("PP", util::get_pp(actual_pp, round(max_pp)), true),
            ("Combo", util::get_combo(&score, &map), true),
            ("Hits", util::get_hits(&score, mode), true),
            ("Map Info", util::get_map_info(&map), false),
        ])
        .author(|a| {
            a.icon_url(format!("{}{}", AVATAR_URL, user.user_id))
                .url(format!("{}u/{}", HOMEPAGE, user.user_id))
                .name(format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = user.username,
                    pp = round_and_comma(user.pp_raw),
                    global = user.pp_rank,
                    country = user.country,
                    national = user.pp_country_rank
                ))
        })
}

fn create_user_score_multi(
    embed: &mut CreateEmbed,
    user: Box<User>,
    mode: GameMode,
    map: Box<Beatmap>,
    scores: Vec<Score>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    embed
        .title(&map)
        .url(format!("{}b/{}", HOMEPAGE, map.beatmap_id))
        .thumbnail(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
        .footer(|f| {
            f.icon_url(format!("{}{}", AVATAR_URL, map.creator_id))
                .text(format!("{:?} map by {}", map.approval_status, map.creator))
        })
        .author(|a| {
            a.icon_url(format!("{}{}", AVATAR_URL, user.user_id))
                .url(format!("{}u/{}", HOMEPAGE, user.user_id))
                .name(format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = user.username,
                    pp = round_and_comma(user.pp_raw),
                    global = user.pp_rank,
                    country = user.country,
                    national = user.pp_country_rank
                ))
        });
    for (i, score) in scores.into_iter().enumerate() {
        // TODO: Handle GameMode's differently
        let (mut oppai, max_pp) = match get_oppai(map.beatmap_id, &score, mode) {
            Ok(tuple) => tuple,
            Err(why) => panic!("Something went wrong while using oppai: {}", why),
        };
        let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
        let mut name = format!(
            "**{idx}.** {grade} {mods}\t[{stars}]\t{score}\t({acc})",
            idx = (i + 1).to_string(),
            grade = util::get_grade_completion_mods(&score, mode, &map, cache.clone()),
            mods = util::get_mods(&score.enabled_mods),
            stars = util::get_stars(&map, Some(oppai)),
            score = with_comma_u64(score.score as u64),
            acc = util::get_acc(&score, mode),
        );
        if mode == GameMode::MNA {
            name.push('\t');
            name.push_str(&util::get_keys(&score.enabled_mods, &map));
        }
        let value = format!(
            "{pp}\t[ {combo} ]\t {hits}\t{ago}",
            pp = util::get_pp(actual_pp, round(max_pp)),
            combo = util::get_combo(&score, &map),
            hits = util::get_hits(&score, mode),
            ago = how_long_ago(&score.date)
        );
        embed.field(name, value, false);
    }
    embed
}

fn create_user_map_multi(
    embed: &mut CreateEmbed,
    user: Box<User>,
    mode: GameMode,
    score_maps: Vec<(Score, Beatmap)>,
    indices: Vec<usize>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    embed
        .author(|a| {
            a.icon_url(format!("{}{}.png", FLAG_URL, user.country))
                .url(format!("{}u/{}", HOMEPAGE, user.user_id))
                .name(format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = user.username,
                    pp = round_and_comma(user.pp_raw),
                    global = user.pp_rank,
                    country = user.country,
                    national = user.pp_country_rank
                ))
        })
        .thumbnail(format!("{}{}", AVATAR_URL, user.user_id));
    let mut description = String::with_capacity(512);
    for ((score, map), idx) in score_maps.iter().zip(indices.iter()) {
        // TODO: Handle GameMode's differently
        let (oppai, max_pp) = match get_oppai(map.beatmap_id, &score, mode) {
            Ok(tuple) => tuple,
            Err(why) => panic!("Something went wrong while using oppai: {}", why),
        };
        let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
        description.push_str(&format!(
            "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
             {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
            idx = idx,
            title = map.title,
            version = map.version,
            base = HOMEPAGE,
            id = map.beatmap_id,
            mods = util::get_mods(&score.enabled_mods),
            stars = util::get_stars(&map, Some(oppai)),
            grade = get_grade_emote(score.grade, cache.clone()),
            pp = util::get_pp(actual_pp, max_pp),
            acc = util::get_acc(&score, mode),
            score = with_comma_u64(score.score as u64),
            combo = util::get_combo(&score, &map),
            hits = util::get_hits(&score, mode),
            ago = how_long_ago(&score.date),
        ));
        description.push('\n');
    }
    description.pop();
    embed.description(description)
}

fn create_profile(
    embed: &mut CreateEmbed,
    u: Box<User>,
    mode: GameMode,
    tuples: Vec<(Score, Beatmap)>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    let bonus_pow =
        0.9994_f64.powi((u.count_ssh + u.count_ss + u.count_sh + u.count_s + u.count_a) as i32);
    let bonus_pp = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
    let values = ProfileResult::calc(mode, tuples);
    let mut combo = String::from(&values.avg_combo.to_string());
    match mode {
        GameMode::STD | GameMode::CTB => {
            combo.push('/');
            combo.push_str(&values.map_combo.to_string());
        }
        _ => {}
    }
    combo.push_str(&format!(" [{} - {}]", values.min_combo, values.max_combo));
    embed
        .author(|a| {
            a.icon_url(format!("{}{}.png", FLAG_URL, u.country))
                .url(format!("{}u/{}", HOMEPAGE, u.user_id))
                .name(format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = u.username,
                    pp = round_and_comma(u.pp_raw),
                    global = u.pp_rank,
                    country = u.country,
                    national = u.pp_country_rank
                ))
        })
        .thumbnail(format!("{}{}", AVATAR_URL, u.user_id))
        .footer(|f| {
            f.text(format!(
                "Joined osu! {} ({})",
                date_to_string(&u.join_date),
                how_long_ago(&u.join_date),
            ))
        })
        .field("Ranked score:", with_comma_u64(u.ranked_score), true)
        .field("Total score:", with_comma_u64(u.total_score), true)
        .field("Total hits:", with_comma_u64(u.get_total_hits()), true)
        .field(
            "Play count / time:",
            format!(
                "{} / {} hrs",
                with_comma_u64(u.playcount as u64),
                u.total_seconds_played / 3600
            ),
            true,
        )
        .field("Level:", round(u.level), true)
        .field("Bonus PP:", format!("~{}pp", bonus_pp), true)
        .field("Accuracy:", format!("{}%", round(u.accuracy)), true)
        .field(
            "Unweighted accuracy:",
            format!(
                "{}% [{}%  {}%]",
                round(values.avg_acc),
                round(values.min_acc),
                round(values.max_acc)
            ),
            true,
        )
        .field(
            "Grades:",
            format!(
                "{xh} {x} {sh} {s} {a}",
                xh = u.count_ssh,
                x = u.count_ss,
                sh = u.count_sh,
                s = u.count_s,
                a = u.count_a
            ),
            false,
        )
        .field(
            "Average PP:",
            format!(
                "{}pp [{} - {}]",
                round(values.avg_pp),
                round(values.min_pp),
                round(values.max_pp)
            ),
            true,
        )
        .field("Average Combo:", combo, true);
    if let Some(mod_combs_count) = values.mod_combs_count {
        embed.field(
            "Favourite mod combinations:",
            mod_combs_count
                .into_iter()
                .map(|(mods, count)| format!("{} {}%", mods, count))
                .join(" > "),
            false,
        );
    }
    embed.field(
        "Favourite mods:",
        values
            .mods_count
            .into_iter()
            .map(|(mods, count)| format!("{} {}%", mods, count))
            .join(" > "),
        false,
    );
    if let Some(mod_combs_pp) = values.mod_combs_pp {
        embed.field(
            "PP earned with mod combination:",
            mod_combs_pp
                .into_iter()
                .map(|(mods, pp)| format!("{} {}pp", mods, round(pp)))
                .join(" > "),
            false,
        );
    }
    embed
        .field(
            "PP earned with mod:",
            values
                .mods_pp
                .into_iter()
                .map(|(mods, pp)| format!("{} {}pp", mods, round(pp)))
                .join(" > "),
            false,
        )
        .field(
            "Mappers in top 100:",
            values
                .mappers
                .into_iter()
                .map(|(name, count, pp)| format!("{}: {}pp ({})", name, round(pp), count))
                .join("\n"),
            true,
        )
        .field(
            "Average map length:",
            format!(
                "{} [{} - {}]",
                sec_to_minsec(values.avg_len),
                sec_to_minsec(values.min_len),
                sec_to_minsec(values.max_len)
            ),
            true,
        )
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
    mods_count: Vec<(GameMod, u32)>,
    mods_pp: Vec<(GameMod, f32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, tuples: Vec<(Score, Beatmap)>) -> Self {
        let (mut min_acc, mut max_acc, mut avg_acc) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_pp, mut max_pp, mut avg_pp) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_combo, mut max_combo, mut avg_combo, mut map_combo) = (u32::MAX, 0, 0, 0);
        let (mut min_len, mut max_len, mut avg_len) = (u32::MAX, 0, 0);
        let len = tuples.len() as f32;
        let mut mappers = HashMap::with_capacity(len as usize);
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        for (score, map) in tuples {
            let acc = score.get_accuracy(mode);
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

            min_len = min_len.min(map.seconds_drain);
            max_len = max_len.max(map.seconds_drain);
            avg_len += map.seconds_drain;

            let mut mapper = *mappers.entry(map.creator).or_insert((0, 0.0));
            let mut mod_comb = *mod_combs
                .entry(score.enabled_mods.clone())
                .or_insert((0, 0.0));
            mapper.0 += 1;
            mod_comb.0 += 1;
            let pp = score.pp.unwrap_or(0.0);
            mapper.1 += pp;
            mod_comb.1 += pp;
            if score.enabled_mods.is_empty() {
                let mut nm = mods.entry(GameMod::NoMod).or_insert((0, 0.0));
                nm.0 += 1;
                nm.1 += pp;
            } else {
                for m in score.enabled_mods {
                    let mut r#mod = mods.entry(m).or_insert((0, 0.0));
                    r#mod.0 += 1;
                    r#mod.1 += pp;
                }
            }
        }
        avg_acc /= len;
        avg_pp /= len;
        avg_combo /= len as u32;
        avg_len /= len as u32;
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
        let (mod_combs_count, mod_combs_pp) = if mod_combs.len() == mods.len() {
            (None, None)
        } else {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (name.clone(), *count))
                .collect();
            mod_combs_count.sort_by(|a, b| b.1.cmp(&a.1));
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();
            mod_combs_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
            (Some(mod_combs_count), Some(mod_combs_pp))
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
            min_len,
            max_len,
            avg_len,
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
            mods_pp,
        }
    }
}

// TODO
fn create_simulation(
    embed: &mut CreateEmbed,
    mode: GameMode,
    score: Option<Box<Score>>,
    map: Box<Beatmap>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    embed
}

// TODO
fn create_ratio(embed: &mut CreateEmbed, user: Box<User>, scores: Vec<Score>) -> &mut CreateEmbed {
    embed
}

// TODO
fn create_leaderboard(
    embed: &mut CreateEmbed,
    mode: GameMode,
    map: Box<Beatmap>,
    tuples: Vec<(User, Score)>,
    cache: CacheRwLock,
) -> &mut CreateEmbed {
    embed
}

// TODO
fn create_common(
    embed: &mut CreateEmbed,
    users: Vec<User>,
    maps: Vec<Beatmap>,
) -> &mut CreateEmbed {
    embed
}
