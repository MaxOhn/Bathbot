use std::{borrow::Cow, str::FromStr};

use regex::Regex;
use rosu_v2::prelude::{GameMode, GameMods, UserId as OsuUserId};
use twilight_model::id::{
    marker::{ChannelMarker, RoleMarker, UserMarker},
    Id,
};

use super::{
    constants::{
        common_literals::{FRUITS, MANIA, OSU, TAIKO},
        OSU_BASE,
    },
    osu::{MapIdType, ModSelection},
};

pub fn is_custom_emote(msg: &str) -> bool {
    EMOJI_MATCHER.is_match(msg)
}

enum MentionType {
    Channel,
    Role,
    User,
}

pub fn get_mention_channel(msg: &str) -> Option<Id<ChannelMarker>> {
    get_mention(MentionType::Channel, msg).and_then(Id::new_checked)
}

pub fn get_mention_role(msg: &str) -> Option<Id<RoleMarker>> {
    get_mention(MentionType::Role, msg).and_then(Id::new_checked)
}

pub fn get_mention_user(msg: &str) -> Option<Id<UserMarker>> {
    msg.parse::<u64>()
        .is_err()
        .then(|| get_mention(MentionType::User, msg))
        .flatten()
        .and_then(Id::new_checked)
}

fn get_mention(mention_type: MentionType, msg: &str) -> Option<u64> {
    if let Ok(id) = msg.parse() {
        return Some(id);
    }

    let captures = match mention_type {
        MentionType::Channel => CHANNEL_ID_MATCHER.captures(msg),
        MentionType::Role => ROLE_ID_MATCHER.captures(msg),
        MentionType::User => MENTION_MATCHER.captures(msg),
    };

    captures
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse().ok())
}

#[allow(dead_code)]
pub fn get_osu_user_id(msg: &str) -> Option<OsuUserId> {
    OSU_URL_USER_MATCHER.captures(msg).and_then(|c| {
        c.get(1)
            .and_then(|m| m.as_str().parse().ok())
            .map(OsuUserId::Id)
            .or_else(|| c.get(2).map(|m| OsuUserId::Name(m.as_str().into())))
    })
}

pub fn get_osu_map_id(msg: &str) -> Option<MapIdType> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(MapIdType::Map(id));
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    let matcher = if let Some(c) = OSU_URL_MAP_OLD_MATCHER.captures(msg) {
        c.get(1)
    } else {
        OSU_URL_MAP_NEW_MATCHER.captures(msg).and_then(|c| c.get(2))
    };

    matcher.and_then(|c| c.as_str().parse::<u32>().ok().map(MapIdType::Map))
}

pub fn get_osu_mapset_id(msg: &str) -> Option<MapIdType> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(MapIdType::Set(id));
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    OSU_URL_MAPSET_OLD_MATCHER
        .captures(msg)
        .or_else(|| OSU_URL_MAP_NEW_MATCHER.captures(msg))
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
        .map(MapIdType::Set)
}

pub fn get_osu_score_id(msg: &str) -> Option<(GameMode, u64)> {
    OSU_SCORE_URL_MATCHER
        .captures(msg)
        .and_then(|c| c.get(1).zip(c.get(2)))
        .and_then(|(mode, id)| {
            let mode = match mode.as_str() {
                OSU => GameMode::STD,
                TAIKO => GameMode::TKO,
                FRUITS => GameMode::CTB,
                MANIA => GameMode::MNA,
                _ => return None,
            };

            let id = id.as_str().parse().ok()?;

            Some((mode, id))
        })
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
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        if msg.ends_with('!') {
            ModSelection::Exact(mods)
        } else {
            ModSelection::Include(mods)
        }
    } else if let Some(captures) = MOD_MINUS_MATCHER.captures(msg) {
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        ModSelection::Exclude(mods)
    } else {
        return None;
    };

    Some(selection)
}

#[allow(dead_code)]
pub fn is_hit_results(msg: &str) -> bool {
    HIT_RESULTS_MATCHER.is_match(msg)
}

pub fn is_guest_diff(msg: &str) -> bool {
    OSU_DIFF_MATCHER.is_match(msg)
}

pub fn tourney_badge(description: &str) -> bool {
    !IGNORE_BADGE_MATCHER.is_match_at(description, 0)
}

pub fn highlight_funny_numeral(content: &str) -> Cow<'_, str> {
    SEVEN_TWO_SEVEN.replace_all(content, "__${num}__")
}

lazy_static! {
    static ref ROLE_ID_MATCHER: Regex = Regex::new(r"<@&(\d+)>").unwrap();

    static ref CHANNEL_ID_MATCHER: Regex = Regex::new(r"<#(\d+)>").unwrap();

    static ref MENTION_MATCHER: Regex = Regex::new(r"<@!?(\d+)>").unwrap();

    static ref OSU_URL_USER_MATCHER: Regex = Regex::new(r"^https://osu.ppy.sh/u(?:sers)?/(?:(\d+)|(\w+))$").unwrap();

    static ref OSU_URL_MAP_NEW_MATCHER: Regex = Regex::new(
        r"https://osu.ppy.sh/beatmapsets/(\d+)(?:(?:#(?:osu|mania|taiko|fruits)|<#\d+>)/(\d+))?"
    )
    .unwrap();

    static ref OSU_URL_MAP_OLD_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/b(?:eatmaps)?/(\d+)").unwrap();
    static ref OSU_URL_MAPSET_OLD_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/s/(\d+)").unwrap();

    static ref OSU_URL_MATCH_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/(?:community/matches|mp)/(\d+)").unwrap();

    static ref MOD_PLUS_MATCHER: Regex = Regex::new(r"^\+(\w+)!?$").unwrap();
    static ref MOD_MINUS_MATCHER: Regex = Regex::new(r"^-(\w+)!$").unwrap();

    static ref HIT_RESULTS_MATCHER: Regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();

    static ref OSU_DIFF_MATCHER: Regex =
        Regex::new(".*'s? (easy|normal|hard|insane|expert|extra|extreme|emotions|repetition)")
            .unwrap();

    static ref EMOJI_MATCHER: Regex = Regex::new(r"<(a?):([^:\n]+):(\d+)>").unwrap();

    static ref IGNORE_BADGE_MATCHER: Regex = Regex::new(r"^((?i)contrib|nomination|assessment|global|moderation|beatmap|spotlight|map|pending|aspire|elite|monthly|exemplary|outstanding|longstanding|idol[^@]+)").unwrap();

    static ref SEVEN_TWO_SEVEN: Regex = Regex::new("(?P<num>7[.,]?2[.,]?7)").unwrap();

    static ref OSU_SCORE_URL_MATCHER: Regex = Regex::new(r"https://osu.ppy.sh/scores/(osu|taiko|mania|fruits)/(\d+)").unwrap();

    pub static ref QUERY_SYNTAX_REGEX: Regex = Regex::new(r#"\b(?P<key>\w+)(?P<op>(:|=|(>|<)(:|=)?))(?P<value>("".*"")|(\S*))"#).unwrap();
}
