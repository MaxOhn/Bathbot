use super::{constants::OSU_BASE, osu::ModSelection};

use lazy_static::lazy_static;
use regex::Regex;
use rosu::models::GameMods;
use std::convert::TryFrom;
use twilight::model::channel::embed::EmbedField;

enum MentionType {
    Channel,
    Role,
    User,
}

pub struct EmojiInfo {
    pub animated: bool,
    pub name: String,
    pub id: u64,
}

pub fn get_emoji_parts(msg: &str) -> Vec<EmojiInfo> {
    if !EMOJI_MATCHER.is_match(msg) {
        return vec![];
    }
    let mut results: Vec<EmojiInfo> = vec![];
    for m in EMOJI_MATCHER.captures_iter(msg) {
        results.push(EmojiInfo {
            animated: &m[0] == "a",
            name: m[1].to_owned(),
            id: m[3].parse::<u64>().unwrap(),
        });
    }
    results
}

pub fn get_mention_channel(msg: &str) -> Option<u64> {
    get_mention(MentionType::Channel, msg)
}

pub fn get_mention_role(msg: &str) -> Option<u64> {
    get_mention(MentionType::Role, msg)
}

pub fn get_mention_user(msg: &str) -> Option<u64> {
    get_mention(MentionType::User, msg)
}

fn get_mention(mention_type: MentionType, msg: &str) -> Option<u64> {
    if let Ok(id) = msg.parse::<u64>() {
        return Some(id);
    }
    let captures = match mention_type {
        MentionType::Channel => CHANNEL_ID_MATCHER.captures(msg),
        MentionType::Role => ROLE_ID_MATCHER.captures(msg),
        MentionType::User => MENTION_MATCHER.captures(msg),
    };
    captures
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u64>().ok())
}

pub fn get_osu_map_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(id);
    }
    if !msg.contains(OSU_BASE) {
        return None;
    }
    let matcher = if let Some(c) = OSU_URL_MAP_OLD_MATCHER.captures(msg) {
        c.get(1)
    } else {
        OSU_URL_MAP_NEW_MATCHER.captures(msg).and_then(|c| c.get(2))
    };
    matcher.and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_osu_mapset_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(id);
    }
    if !msg.contains(OSU_BASE) {
        return None;
    }
    OSU_URL_MAPSET_OLD_MATCHER
        .captures(msg)
        .or_else(|| OSU_URL_MAP_NEW_MATCHER.captures(msg))
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_osu_match_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(id);
    }
    OSU_URL_MATCH_MATCHER
        .captures(msg)
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_mods(msg: &str) -> Option<ModSelection> {
    let selection = if let Some(captures) = MOD_PLUS_MATCHER.captures(msg) {
        let mods = GameMods::try_from(captures.get(1)?.as_str()).ok()?;
        if msg.ends_with('!') {
            ModSelection::Exact(mods)
        } else {
            ModSelection::Include(mods)
        }
    } else if let Some(captures) = MOD_MINUS_MATCHER.captures(msg) {
        let mods = GameMods::try_from(captures.get(1)?.as_str()).ok()?;
        ModSelection::Exclude(mods)
    } else {
        return None;
    };
    Some(selection)
}

pub fn is_hit_results(msg: &str) -> bool {
    HIT_RESULTS_MATCHER.is_match(msg)
}

lazy_static! {
    static ref EMOJI_MATCHER: Regex = Regex::new(r"<(a?):([^:\n]+):(\d+)>").unwrap();
}

lazy_static! {
    static ref ROLE_ID_MATCHER: Regex = Regex::new(r"<@&(\d+)>").unwrap();
}

lazy_static! {
    static ref CHANNEL_ID_MATCHER: Regex = Regex::new(r"<#(\d+)>").unwrap();
}

lazy_static! {
    static ref MENTION_MATCHER: Regex = Regex::new(r"<@!?(\d+)>").unwrap();
}

lazy_static! {
    static ref OSU_URL_MAP_NEW_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/beatmapsets/(\d+)#[osu|mania|taiko|fruits]/(\d+)").unwrap();
}

lazy_static! {
    static ref OSU_URL_MAP_OLD_MATCHER: Regex = Regex::new(r"https://osu.ppy.sh/b/(\d+)").unwrap();
    static ref OSU_URL_MAPSET_OLD_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/s/(\d+)").unwrap();
}

lazy_static! {
    static ref OSU_URL_MATCH_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/community/matches/(\d+)").unwrap();
}

lazy_static! {
    static ref MOD_PLUS_MATCHER: Regex = Regex::new(r"^+(\w+)!?$").unwrap();
    static ref MOD_MINUS_MATCHER: Regex = Regex::new(r"^-(\w+)!$").unwrap();
}

lazy_static! {
    static ref HIT_RESULTS_MATCHER: Regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();
}
