use std::{borrow::Cow, sync::LazyLock};

use regex::Regex;
use rosu_v2::prelude::{GameMode, GameModsIntermode, UserId as OsuUserId};
use twilight_model::id::{
    marker::{RoleMarker, UserMarker},
    Id,
};

use super::osu::ModSelection;

pub fn is_approved_skin_site(url: &str) -> bool {
    APPROVED_SKIN_SITE.is_match(url)
}

pub fn is_custom_emote(msg: &str) -> bool {
    EMOJI_MATCHER.is_match(msg)
}

enum MentionType {
    Role,
    User,
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

pub fn get_osu_map_id(msg: &str) -> Option<u32> {
    if let Some(id) = msg.parse().ok().filter(|_| !msg.starts_with('+')) {
        return Some(id);
    }

    let matcher = if let Some(c) = OSU_URL_MAP_OLD_MATCHER.captures(msg) {
        c.get(1)
    } else {
        OSU_URL_MAP_NEW_MATCHER.captures(msg).and_then(|c| c.get(2))
    };

    matcher.and_then(|c| c.as_str().parse().ok())
}

pub fn get_single_osu_map_id(msg: &str) -> Option<u32> {
    let count_old = OSU_URL_MAP_OLD_MATCHER.find_iter(msg).count();
    let count_new = OSU_URL_MAP_NEW_MATCHER.find_iter(msg).count();

    (count_old + count_new == 1)
        .then(|| get_osu_map_id(msg))
        .flatten()
}

pub fn get_osu_mapset_id(msg: &str) -> Option<u32> {
    if let Some(id) = msg.parse().ok().filter(|_| !msg.starts_with('+')) {
        return Some(id);
    }

    OSU_URL_MAPSET_OLD_MATCHER
        .captures(msg)
        .or_else(|| OSU_URL_MAP_NEW_MATCHER.captures(msg))
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse().ok())
}

pub fn get_osu_score_id(msg: &str) -> Option<(u64, Option<GameMode>)> {
    OSU_SCORE_URL_MATCHER
        .captures(msg)
        .and_then(|c| c.get(2).map(|x| (x, c.get(1))))
        .and_then(|(id, mode)| {
            let mode = mode.map(|mode| match mode.as_str() {
                "osu" => GameMode::Osu,
                "taiko" => GameMode::Taiko,
                "fruits" => GameMode::Catch,
                "mania" => GameMode::Mania,
                _ => unreachable!(),
            });

            let id = id.as_str().parse().ok()?;

            Some((id, mode))
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
        let mods = GameModsIntermode::try_from_acronyms(captures.get(1)?.as_str())?;

        if msg.ends_with('!') {
            ModSelection::Exact(mods)
        } else {
            ModSelection::Include(mods)
        }
    } else if let Some(captures) = MOD_MINUS_MATCHER.captures(msg) {
        let mods = GameModsIntermode::try_from_acronyms(captures.get(1)?.as_str())?;

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

pub fn highlight_funny_numeral(content: &str) -> Cow<'_, str> {
    SEVEN_TWO_SEVEN.replace_all(content, "__${num}__")
}

macro_rules! define_regex {
    ( $( $vis:vis $name:ident: $pat:literal; )* ) => {
        $(
            $vis static $name: LazyLock<Regex> =
                LazyLock::new(|| Regex::new($pat).unwrap());
        )*
    }
}

define_regex! {
    ROLE_ID_MATCHER: r"<@&(\d+)>";
    MENTION_MATCHER: r"<@!?(\d+)>";

    OSU_URL_USER_MATCHER: r"^https://osu\.ppy\.sh/u(?:sers)?/(?:(\d+)|(\w+))$";

    OSU_URL_MAP_NEW_MATCHER: r"https://osu\.ppy\.sh/beatmapsets/(\d+)(?:(?:/?#(?:osu|mania|taiko|fruits)|<#\d+>)/(\d+))?";
    OSU_URL_MAP_OLD_MATCHER: r"https://osu\.ppy\.sh/b(?:eatmaps)?/(\d+)";
    OSU_URL_MAPSET_OLD_MATCHER: r"https://osu\.ppy\.sh/s/(\d+)";

    OSU_URL_MATCH_MATCHER: r"https://osu\.ppy\.sh/(?:community/matches|mp)/(\d+)";

    MOD_PLUS_MATCHER: r"^\+(\w+)!?$";
    MOD_MINUS_MATCHER: r"^-(\w+)!$";

    HIT_RESULTS_MATCHER: r".*\{(\d+/){2,}\d+}.*";

    EMOJI_MATCHER: r"<(a?):([^:\n]+):(\d+)>";

    SEVEN_TWO_SEVEN: "(?P<num>7[.,]?2[.,]?7)";

    OSU_SCORE_URL_MATCHER: r"https://osu\.ppy\.sh/scores/(?:(osu|taiko|mania|fruits)/)?(\d+)";

    APPROVED_SKIN_SITE: r"^https://(?:(?:www\.)?(?:drive\.google\.com|dropbox\.com|mega\.nz|mediafire\.com|(?:gist\.)?github\.com)/.*$|(?:skins\.osuck\.net/skins|osu\.ppy\.sh/community/forums/topics)/\d+.*|link.issou.best/skin/\d+$)";

    pub QUERY_SYNTAX_REGEX: r#"\b(?P<key>\w+)(?P<op>(:|=|(>|<)(:|=)?))(?P<value>(".*")|(\S*))"#;
}
