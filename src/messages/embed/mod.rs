#![allow(clippy::too_many_arguments)]
#![allow(unused)]

mod util;

use crate::util::{
    datetime::{date_to_string, how_long_ago},
    numbers::{round, round_and_comma, with_comma_u32},
    osu::*,
};

use roppai::Oppai;
use rosu::models::{Beatmap, GameMod, GameMode, Score, User};
use serenity::{builder::CreateEmbed, cache::CacheRwLock, utils::Colour};

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
    SimulateScore(Box<Score>, Box<Beatmap>),
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
            Profile(user, tuples) => e,
            SimulateScore(score, map) => e,
            UserLeaderboard(map, tuples) => e,
            ManiaRatio(user, scores) => e,
            UserCommonScores(users, maps) => e,
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
        .timestamp(date_to_string(score.date))
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
            ("Score", with_comma_u32(score.score), true),
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
            score = with_comma_u32(score.score),
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
            score = with_comma_u32(score.score),
            combo = util::get_combo(&score, &map),
            hits = util::get_hits(&score, mode),
            ago = how_long_ago(&score.date),
        ));
        description.push('\n');
    }
    description.pop();
    embed.description(description)
}
