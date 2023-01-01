/// Only checks for non-uppercase characters
pub fn contains_disallowed_infix(haystack: impl Into<String>) -> bool {
    const DISALLOWED_INFIX: &[&str] = &[
        "qfqqz",
        "dppljf{",
        "difbu",
        "ojhhfs",
        "mpmj",
        "gvdl",
        "ejmep",
        "gbhhpu",
        "dvou",
        "tijhfupsb",
        "qpso",
        "cbodip",
        "qfojt",
        "wbhjob",
        "qvttz",
        "ejdl",
        "dpdl",
        "brvjmb",
        "ijumfs",
        "ibdl",
    ];

    const MAX_NEEDLE_LEN: usize = 9;

    let mut bad_chars = [0; 256];
    let mut good_suff = [0; MAX_NEEDLE_LEN];

    let mut haystack = haystack.into();

    for byte in unsafe { haystack.as_mut_vec().iter_mut() } {
        *byte += 1;
    }

    DISALLOWED_INFIX.iter().any(|&needle| {
        is_substring_boyer_moore::<MAX_NEEDLE_LEN>(
            needle,
            &haystack,
            &mut bad_chars,
            &mut good_suff,
        )
    })
}

/// https://en.wikipedia.org/wiki/Boyer%E2%80%93Moore_string-search_algorithm
fn is_substring_boyer_moore<const MAX_NEEDLE_LEN: usize>(
    needle: &str,
    haystack: &str,
    bad_chars: &mut [usize],
    good_suff: &mut [usize],
) -> bool {
    let needle_len = needle.len();
    let haystack_len = haystack.len();

    // empty pattern must be considered specially
    if needle_len == 0 || needle_len > haystack_len {
        return false;
    }

    let needle = needle.as_bytes();
    let haystack = haystack.as_bytes();

    pre_bad_char(needle, bad_chars);
    pre_good_suff(needle, good_suff);

    let mut i = needle_len - 1; // haystack idx

    while i < haystack_len {
        let mut j = needle_len - 1; // needle idx

        while haystack[i] == needle[j] {
            if let Some(j_) = j.checked_sub(1) {
                j = j_;
                i -= 1;
            } else {
                return true;
            }
        }

        let shift = good_suff[j].max(bad_chars[haystack[i] as usize]);
        i += shift;
    }

    false
}

fn pre_bad_char(needle: &[u8], bad_chars: &mut [usize]) {
    let needle_len = needle.len();

    for elem in bad_chars.iter_mut() {
        *elem = needle_len;
    }

    for i in 0..needle.len() {
        bad_chars[needle[i] as usize] = needle_len - i - 1;
    }
}

// true if the suffix of `word` starting from `pos` is a prefix of `word`
fn is_prefix(word: &[u8], pos: usize) -> bool {
    word[..word.len() - pos] == word[pos..]
}

// length of the longest suffix of `word` ending on `pos`
fn suffix_len(word: &[u8], pos: usize) -> usize {
    let word_front = word.iter().take(pos + 1).rev();
    let word_back = word.iter().rev();

    word_front
        .zip(word_back)
        .take_while(|(front, back)| front == back)
        .count()
}

fn pre_good_suff(needle: &[u8], good_suff: &mut [usize]) {
    let needle_len = needle.len();
    let mut last_prefix_idx = 1;

    for p in (0..needle_len).rev() {
        if is_prefix(needle, p + 1) {
            last_prefix_idx = p + 1;
        }

        good_suff[p] = last_prefix_idx + (needle_len - p - 1);
    }

    for p in 0..needle_len - 1 {
        let suff_len = suffix_len(needle, p);

        if p < suff_len || needle[p - suff_len] != needle[needle_len - suff_len - 1] {
            good_suff[needle_len - suff_len - 1] = needle_len + suff_len - p - 1;
        }
    }
}
