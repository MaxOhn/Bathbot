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

    let mut bad_chars = [0; 256];
    let mut good_suff = [0; 9];

    let mut haystack = haystack.into();

    for byte in unsafe { haystack.as_mut_vec().iter_mut() } {
        *byte += 1;
    }

    DISALLOWED_INFIX.iter().any(|&needle| {
        is_substring_boyer_moore::<9>(needle, &haystack, &mut bad_chars, &mut good_suff)
    })
}

/// http://igm.univ-mlv.fr/~lecroq/string/node14.html#SECTION00140
fn is_substring_boyer_moore<const MAX_NEEDLE_LEN: usize>(
    needle: &str,
    haystack: &str,
    bad_chars: &mut [usize],
    good_suff: &mut [usize],
) -> bool {
    let m = needle.len();
    let n = haystack.len();

    if m > n {
        return false;
    }

    let needle = needle.as_bytes();
    let haystack = haystack.as_bytes();

    pre_bad_char(needle, bad_chars);
    pre_good_suff::<MAX_NEEDLE_LEN>(needle, good_suff);

    let mut j = 0;

    while j <= n - m {
        let mut i = m - 1;

        while needle[i] == haystack[i + j] {
            if let Some(i_) = i.checked_sub(1) {
                i = i_;
            } else {
                return true;
            }
        }

        j += good_suff[i].max(bad_chars[haystack[i + j] as usize] + 1 + i - m);
    }

    false
}

fn pre_bad_char(pat: &[u8], bad_chars: &mut [usize]) {
    let m = pat.len();

    for i in 0..256 {
        bad_chars[i] = m;
    }

    for i in 0..pat.len() - 1 {
        bad_chars[pat[i] as usize] = m - i - 1;
    }
}

fn pre_good_suff<const MAX_NEEDLE_LEN: usize>(pat: &[u8], good_suff: &mut [usize]) {
    let m = pat.len();
    let mut suff = vec![0; MAX_NEEDLE_LEN];

    suffixes(pat, &mut suff);

    for i in 0..m {
        good_suff[i] = m;
    }

    let mut j = 0;

    for i in (0..=m - 1).rev() {
        if suff[i] == i + 1 {
            while j < m - 1 - i {
                if good_suff[j] == m {
                    good_suff[j] = m - 1 - i;
                }

                j += 1;
            }
        }
    }

    for i in 0..=m - 2 {
        good_suff[m - 1 - suff[i]] = m - 1 - i;
    }
}

fn suffixes(pat: &[u8], suff: &mut [usize]) {
    let m = pat.len();

    suff[m - 1] = m;

    let mut g = m - 1;
    let mut f = 0;

    for i in (0..=m - 2).rev() {
        if i > g && suff[i + m - 1 - f] < i - g {
            suff[i] = suff[i + m - 1 - f];
        } else {
            if i < g {
                g = i;
            }

            f = i;

            while pat[g] == pat[g + m - 1 - f] {
                if let Some(g_) = g.checked_sub(1) {
                    g = g_;
                } else {
                    f += 1;
                    break;
                }
            }

            suff[i] = f - g;
        }
    }
}
