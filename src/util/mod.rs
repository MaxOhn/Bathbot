pub mod bg_game;
pub mod constants;
pub mod error;
pub mod matcher;

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
