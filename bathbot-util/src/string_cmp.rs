macro_rules! get {
    ($slice:ident[$idx:expr]) => {
        unsafe { *$slice.get_unchecked($idx) }
    };
}

macro_rules! set {
    ($slice:ident[$idx:expr] = $val:expr) => {
        unsafe { *$slice.get_unchecked_mut($idx) = $val }
    };
}

use std::mem;

pub fn levenshtein_similarity(word_a: &str, word_b: &str) -> f32 {
    let (dist, len) = levenshtein_distance(word_a, word_b);

    (len - dist) as f32 / len as f32
}

/// "How many replace/delete/insert operations are necessary to morph one word
/// into the other?"
///
/// Returns (distance, max word length) tuple
pub fn levenshtein_distance<'w>(mut word_a: &'w str, mut word_b: &'w str) -> (usize, usize) {
    let m = word_a.chars().count();
    let mut n = word_b.chars().count();

    if m > n {
        mem::swap(&mut word_a, &mut word_b);
        n = m;
    }

    // u16 is sufficient considering the max length
    // of discord messages is smaller than u16::MAX
    let mut costs: Vec<_> = (0..=n as u16).collect();

    // SAFETY for get! and set!:
    // chars(word_a) <= chars(word_b) = n < n + 1 = costs.len()

    for (a, i) in word_a.chars().zip(1..) {
        let mut last_val = i;

        for (b, j) in word_b.chars().zip(1..) {
            let new_val = if a == b {
                get!(costs[j - 1])
            } else {
                get!(costs[j - 1]).min(last_val).min(get!(costs[j])) + 1
            };

            set!(costs[j - 1] = last_val);
            last_val = new_val;
        }

        set!(costs[n] = last_val);
    }

    (get!(costs[n]) as usize, n)
}

/// Consider the length of the longest common substring, then repeat recursively
/// for the remaining left and right parts of the words
pub fn gestalt_pattern_matching(word_a: &str, word_b: &str) -> f32 {
    let chars_a = word_a.chars().count();
    let chars_b = word_b.chars().count();

    // u16 is sufficient considering the max length
    // of discord messages is smaller than u16::MAX
    let mut buf = vec![0; chars_a.max(chars_b) + 1];

    // SAFETY: buf.len is set to be 1 + max(chars(word_a), chars(word_b))
    let matching_chars = unsafe { _gestalt_pattern_matching(word_a, word_b, &mut buf) };

    (2 * matching_chars) as f32 / (chars_a + chars_b) as f32
}

/// Caller must guarantee that buf.len is 1 + max(chars(word_a), chars(word_b))
unsafe fn _gestalt_pattern_matching(word_a: &str, word_b: &str, buf: &mut [u16]) -> usize {
    let SubstringResult {
        start_a,
        start_b,
        len,
    } = unsafe { longest_common_substring(word_a, word_b, buf) };

    if len == 0 {
        return 0;
    }

    let mut matches = len;

    if start_a > 0 && start_b > 0 {
        let prefix_a = prefix(word_a, start_a);
        let prefix_b = prefix(word_b, start_b);
        matches += unsafe { _gestalt_pattern_matching(prefix_a, prefix_b, buf) };
    }

    let suffix_a = suffix(word_a, start_a + len);
    let suffix_b = suffix(word_b, start_b + len);

    if !(suffix_a.is_empty() || suffix_b.is_empty()) {
        matches += unsafe { _gestalt_pattern_matching(suffix_a, suffix_b, buf) };
    }

    matches
}

fn prefix(s: &str, len: usize) -> &str {
    let mut indices = s.char_indices();
    let end = indices.nth(len).map_or_else(|| s.len(), |(i, _)| i);

    // SAFETY: `end` is provided by `char_indices` which ensues valid char bounds
    unsafe { s.get_unchecked(..end) }
}

fn suffix(s: &str, start: usize) -> &str {
    let mut indices = s.char_indices();
    let start = indices.nth(start).map_or_else(|| s.len(), |(i, _)| i);

    // SAFETY: `start` is provided by `char_indices` which ensues valid char bounds
    unsafe { s.get_unchecked(start..) }
}

/// Caller must guarantee that buf.len >= 1 + max(chars(word_a), chars(word_b))
unsafe fn longest_common_substring<'w>(
    mut word_a: &'w str,
    mut word_b: &'w str,
    buf: &mut [u16],
) -> SubstringResult {
    if word_a.is_empty() || word_b.is_empty() {
        return SubstringResult::default();
    }

    let mut swapped = false;
    let mut m = word_a.chars().count();
    let mut n = word_b.chars().count();

    // Ensure word_b being the longer word with length n
    if m > n {
        mem::swap(&mut word_a, &mut word_b);
        mem::swap(&mut m, &mut n);
        swapped = true;
    }

    let mut len = 0;
    let mut start_b = 0;
    let mut end_a = 0;

    // SAFETY for indices:
    // i ranges from 0 to n - 1 so the indices range from 0 to n
    // No issue since buf.len = n + 1, as guaranteed by the caller

    for (j, a) in word_a.chars().rev().enumerate() {
        for (i, b) in word_b.chars().enumerate() {
            if a != b {
                unsafe { *buf.get_unchecked_mut(i) = 0 };

                continue;
            }

            let val = unsafe { *buf.get_unchecked(i + 1) + 1 };
            unsafe { *buf.get_unchecked_mut(i) = val };

            if val > len {
                len = val;
                start_b = i;
                end_a = j;
            }
        }
    }

    let (start_a, start_b) = if swapped {
        (start_b, m - end_a - 1)
    } else {
        (m - end_a - 1, start_b)
    };

    // Reset the buffer
    for elem in buf.iter_mut().take(n) {
        *elem = 0;
    }

    SubstringResult {
        start_a,
        start_b,
        len: len as usize,
    }
}

#[derive(Default)]
struct SubstringResult {
    start_a: usize,
    start_b: usize,
    len: usize,
}
