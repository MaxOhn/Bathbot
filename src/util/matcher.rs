use lazy_static::lazy_static;
use regex::Regex;

pub fn contains_emote(msg: &str) -> bool {
    EMOJI_MATCHER.is_match(msg)
}

pub struct EmojiInfo {
    pub animated: bool,
    pub name: String,
    pub id: u64,
}

pub fn get_emoji_parts(msg: &str) -> Vec<EmojiInfo> {
    if !contains_emote(msg) {
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

lazy_static! {
    static ref EMOJI_MATCHER: Regex = Regex::new(r"<(a?):([^:\n]+):([0-9]+)>").unwrap();
}
