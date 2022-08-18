use std::{borrow::Cow, str::FromStr};

use once_cell::sync::OnceCell;
use rosu_v2::prelude::{GameMode, GameMods, UserId as OsuUserId};
use twilight_model::id::{
    marker::{ChannelMarker, RoleMarker, UserMarker},
    Id,
};

use super::{constants::OSU_BASE, osu::ModSelection};

pub fn is_custom_emote(msg: &str) -> bool {
    EMOJI_MATCHER.get().is_match(msg)
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
        MentionType::Channel => CHANNEL_ID_MATCHER.get().captures(msg),
        MentionType::Role => ROLE_ID_MATCHER.get().captures(msg),
        MentionType::User => MENTION_MATCHER.get().captures(msg),
    };

    captures
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse().ok())
}

#[allow(dead_code)]
pub fn get_osu_user_id(msg: &str) -> Option<OsuUserId> {
    OSU_URL_USER_MATCHER.get().captures(msg).and_then(|c| {
        c.get(1)
            .and_then(|m| m.as_str().parse().ok())
            .map(OsuUserId::Id)
            .or_else(|| c.get(2).map(|m| OsuUserId::Name(m.as_str().into())))
    })
}

pub fn get_osu_map_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse() {
        return Some(id);
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    let matcher = if let Some(c) = OSU_URL_MAP_OLD_MATCHER.get().captures(msg) {
        c.get(1)
    } else {
        OSU_URL_MAP_NEW_MATCHER
            .get()
            .captures(msg)
            .and_then(|c| c.get(2))
    };

    matcher.and_then(|c| c.as_str().parse().ok())
}

pub fn get_osu_mapset_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse() {
        return Some(id);
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    OSU_URL_MAPSET_OLD_MATCHER
        .get()
        .captures(msg)
        .or_else(|| OSU_URL_MAP_NEW_MATCHER.get().captures(msg))
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse().ok())
}

pub fn get_osu_score_id(msg: &str) -> Option<(GameMode, u64)> {
    OSU_SCORE_URL_MATCHER
        .get()
        .captures(msg)
        .and_then(|c| c.get(1).zip(c.get(2)))
        .and_then(|(mode, id)| {
            let mode = match mode.as_str() {
                "osu" => GameMode::Osu,
                "taiko" => GameMode::Taiko,
                "fruits" => GameMode::Catch,
                "mania" => GameMode::Mania,
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
        .get()
        .captures(msg)
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_mods(msg: &str) -> Option<ModSelection> {
    let selection = if let Some(captures) = MOD_PLUS_MATCHER.get().captures(msg) {
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        if msg.ends_with('!') {
            ModSelection::Exact(mods)
        } else {
            ModSelection::Include(mods)
        }
    } else if let Some(captures) = MOD_MINUS_MATCHER.get().captures(msg) {
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        ModSelection::Exclude(mods)
    } else {
        return None;
    };

    Some(selection)
}

#[allow(dead_code)]
pub fn is_hit_results(msg: &str) -> bool {
    HIT_RESULTS_MATCHER.get().is_match(msg)
}

pub fn tourney_badge(description: &str) -> bool {
    !IGNORE_BADGE_MATCHER.get().is_match_at(description, 0)
}

pub fn highlight_funny_numeral(content: &str) -> Cow<'_, str> {
    SEVEN_TWO_SEVEN.get().replace_all(content, "__${num}__")
}

pub struct Regex {
    regex: &'static str,
    cell: OnceCell<regex::Regex>,
}

impl Regex {
    const fn new(regex: &'static str) -> Self {
        Self {
            regex,
            cell: OnceCell::new(),
        }
    }

    pub fn get(&self) -> &regex::Regex {
        self.cell
            .get_or_init(|| regex::Regex::new(self.regex).unwrap())
    }
}

macro_rules! define_regex {
    ($($name:ident: $pat:literal;)*) => {
        $( static $name: Regex = Regex::new($pat); )*
    }
}

define_regex! {
    ROLE_ID_MATCHER: r"<@&(\d+)>";
    CHANNEL_ID_MATCHER: r"<#(\d+)>";
    MENTION_MATCHER: r"<@!?(\d+)>";

    OSU_URL_USER_MATCHER: r"^https://osu.ppy.sh/u(?:sers)?/(?:(\d+)|(\w+))$";

    OSU_URL_MAP_NEW_MATCHER: r"https://osu.ppy.sh/beatmapsets/(\d+)(?:(?:#(?:osu|mania|taiko|fruits)|<#\d+>)/(\d+))?";
    OSU_URL_MAP_OLD_MATCHER: r"https://osu.ppy.sh/b(?:eatmaps)?/(\d+)";
    OSU_URL_MAPSET_OLD_MATCHER: r"https://osu.ppy.sh/s/(\d+)";

    OSU_URL_MATCH_MATCHER: r"https://osu.ppy.sh/(?:community/matches|mp)/(\d+)";

    MOD_PLUS_MATCHER: r"^\+(\w+)!?$";
    MOD_MINUS_MATCHER: r"^-(\w+)!$";

    HIT_RESULTS_MATCHER: r".*\{(\d+/){2,}\d+}.*";

    EMOJI_MATCHER: r"<(a?):([^:\n]+):(\d+)>";

    IGNORE_BADGE_MATCHER: r"^(?i:contrib|nomination|assessment|global|moderation|beatmap|spotlight|map|pending|aspire|elite|monthly|exemplary|outstanding|longstanding|idol[^@]+)|(?i:fanart contest)";

    SEVEN_TWO_SEVEN: "(?P<num>7[.,]?2[.,]?7)";

    OSU_SCORE_URL_MATCHER: r"https://osu.ppy.sh/scores/(osu|taiko|mania|fruits)/(\d+)";
}

pub static QUERY_SYNTAX_REGEX: Regex =
    Regex::new(r#"\b(?P<key>\w+)(?P<op>(:|=|(>|<)(:|=)?))(?P<value>("".*"")|(\S*))"#);
