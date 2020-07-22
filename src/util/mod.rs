pub mod bg_game;
pub mod constants;
pub mod datetime;
#[macro_use]
pub mod error;
pub mod exts;
pub mod matcher;
pub mod numbers;
pub mod osu;
mod safe_content;

use constants::DISCORD_CDN;
pub use exts::*;
pub use safe_content::content_safe;

use twilight::model::id::UserId;

pub fn discord_avatar(user_id: UserId, hash: &str) -> String {
    format!("{}avatars/{}/{}.webp?size=1024", DISCORD_CDN, user_id, hash)
}

pub fn levenshtein_distance(word_a: &str, word_b: &str) -> usize {
    let (word_a, word_b) = if word_a.chars().count() > word_b.chars().count() {
        (word_b, word_a)
    } else {
        (word_a, word_b)
    };
    let mut costs: Vec<usize> = (0..=word_b.len()).collect();
    for (i, a) in (1..=word_a.len()).zip(word_a.chars()) {
        let mut last_val = i;
        for (j, b) in (1..=word_b.len()).zip(word_b.chars()) {
            let new_val = if a == b {
                costs[j - 1]
            } else {
                costs[j - 1].min(last_val).min(costs[j]) + 1
            };
            costs[j - 1] = last_val;
            last_val = new_val;
        }
        costs[word_b.len()] = last_val;
    }
    costs[word_b.len()]
}
