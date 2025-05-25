use memchr::memchr;

// credits to claude.ai :)
pub fn decode_html_entities(input: &str) -> String {
    static ENTITY_MAP: &[(&str, &str)] = &[
        ("&Tab;", "\t"),
        ("&NewLine;", "\n"),
        ("&excl;", "!"),
        ("&quot;", "\""),
        ("&num;", "#"),
        ("&dollar;", "$"),
        ("&percnt;", "%"),
        ("&amp;", "&"),
        ("&apos;", "'"),
        ("&lpar;", "("),
        ("&rpar;", ")"),
        ("&ast;", "*"),
        ("&plus;", "+"),
        ("&comma;", ","),
        ("&hyphen;", "-"),
        ("&period;", "."),
        ("&sol;", "/"),
        ("&colon;", ":"),
        ("&semi;", ";"),
        ("&lt;", "<"),
        ("&equals;", "="),
        ("&gt;", ">"),
        ("&quest;", "?"),
        ("&commat;", "@"),
        ("&lsqb;", "["),
        ("&bsol;", "\\"),
        ("&rsqb;", "]"),
        ("&Hat;", "^"),
        ("&lowbar;", "_"),
        ("&grave;", "`"),
        ("&lcub;", "{"),
        ("&verbar;", "|"),
        ("&rcub;", "}"),
        ("&tilde;", "~"),
        ("&nbsp;", " "),
    ];

    let bytes = input.as_bytes();
    let mut result = String::with_capacity(input.len());
    let mut pos = 0;

    while let Some(amp_offset) = memchr(b'&', &bytes[pos..]) {
        let amp_pos = pos + amp_offset;

        // Copy all text before the '&'
        result.push_str(&input[pos..amp_pos]);

        // Calculate safe search range for semicolon (max 20 chars after &)
        let search_start = amp_pos + 1;
        let search_end = usize::min(amp_pos + 21, bytes.len());

        // Find semicolon within safe range
        if search_start >= bytes.len() {
            // '&' is at the very end of string
            result.push('&');
            pos = amp_pos + 1;

            continue;
        }

        let Some(semi_offset) = memchr(b';', &bytes[search_start..search_end]) else {
            // No semicolon found - keep the '&' and continue
            result.push('&');
            pos = amp_pos + 1;

            continue;
        };

        let semi_pos = search_start + semi_offset;
        let entity_slice = &input[amp_pos..=semi_pos];

        // Check for entity match
        let mut found = false;

        for &(entity, replacement) in ENTITY_MAP {
            if entity_slice == entity {
                result.push_str(replacement);
                found = true;

                break;
            }
        }

        if !found && entity_slice.len() > 4 && entity_slice.starts_with("&#") {
            // Handle numeric entities
            let number_part = &entity_slice[2..entity_slice.len() - 1];

            let code_res = if let Some(stripped) = number_part.strip_prefix('x') {
                u32::from_str_radix(stripped, 16)
            } else {
                number_part.parse::<u32>()
            };

            if let Ok(code) = code_res {
                if code <= 127 {
                    if let Some(ascii_char) = char::from_u32(code) {
                        result.push(ascii_char);
                        found = true;
                    }
                }
            }
        }

        if found {
            pos = semi_pos + 1; // Skip past the entire entity
        } else {
            // Invalid entity - keep the '&' and continue from next char
            result.push('&');
            pos = amp_pos + 1;
        }
    }

    // Copy any remaining text after the last '&'
    result.push_str(&input[pos..]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_html_entities() {
        fn assert_eq(input: &str, expected: &str) {
            assert_eq!(decode_html_entities(input), expected);
        }

        assert_eq("&lt;tag&gt;", "<tag>");
        assert_eq("&#65;&#66;&#67;", "ABC");
        assert_eq("&#x41;&#x42;&#x43;", "ABC");
        assert_eq("&quot;Hello&quot;", "\"Hello\"");
        assert_eq("Price&colon; &dollar;100", "Price: $100");
        assert_eq("No entities here", "No entities here");
        assert_eq("&invalid;", "&invalid;"); // Unknown entity preserved
    }
}
