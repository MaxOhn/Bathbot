use crate::{
    commands::osu::UserValue,
    embeds::{Author, Footer},
    util::{osu::flag_url, CountryCode},
};

use std::{collections::BTreeMap, fmt::Write};

pub struct OsekaiUserValueEmbed {
    description: String,
    footer: Footer,
    title: &'static str,
}

impl OsekaiUserValueEmbed {
    pub fn new(users: &BTreeMap<usize, (UserValue, String)>, pages: (usize, usize)) -> Self {
        let index = (pages.0 - 1) * 20;

        let mut buf = String::new();

        let left_lengths = lengths(&mut buf, users.range(index..index + 10));
        let right_lengths = lengths(&mut buf, users.range(index + 10..index + 20));

        let mut description = String::with_capacity(1024);

        // Ensuring the right side has ten elements for the zip
        let user_iter = users
            .range(index..index + 10)
            .zip((10..20).map(|i| users.get(&(index + i))));

        for ((i, (left_value, left_name)), right) in user_iter {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", left_value);

            let _ = write!(
                description,
                "`#{idx:<idx_len$}`:flag_{country}:`{name:<name_len$}``{value:>value_len$}`",
                idx = idx,
                idx_len = left_lengths.idx,
                country = "TODO",
                name = left_name,
                name_len = left_lengths.name,
                value = buf,
                value_len = left_lengths.value,
            );

            if let Some((right_value, right_name)) = right {
                buf.clear();
                let _ = write!(buf, "{}", right_value);

                let _ = write!(
                    description,
                    " | `#{idx:<idx_len$}`:flag_{country}:`{name:<name_len$}``{value:>value_len$}`",
                    idx = idx + 10,
                    idx_len = right_lengths.idx,
                    name = right_name,
                    name_len = right_lengths.name,
                    value = buf,
                    value_len = right_lengths.value,
                );
            }

            description.push('\n');
        }

        // let mut author = Author::new(format!("{} Ranking for osu!{}", title, mode_str(mode)));

        // author = if let Some(code) = country_code {
        //     let url = format!(
        //         "https://osu.ppy.sh/rankings/{}/{}?country={}",
        //         mode, url_type, code
        //     );

        //     author.url(url).icon_url(flag_url(code))
        // } else {
        //     author.url(format!("https://osu.ppy.sh/rankings/{}/{}", mode, url_type))
        // };

        let footer_text = format!(
            "Page {}/{} | Check out osekai.net for more info",
            pages.0, pages.1
        );

        Self {
            description,
            footer: Footer::new(footer_text),
            title: "TODO",
        }
    }
}

impl_builder!(OsekaiUserValueEmbed {
    description,
    footer,
    title,
});

fn lengths<'i>(
    buf: &mut String,
    iter: impl Iterator<Item = (&'i usize, &'i (UserValue, String))>,
) -> Lengths {
    let mut idx_len = 0;
    let mut name_len = 0;
    let mut value_len = 0;

    for (i, (value, name)) in iter {
        let mut idx = i + 1;
        let mut len = 0;

        while idx > 0 {
            len += 1;
            idx /= 10;
        }

        idx_len = idx_len.max(len);
        name_len = name_len.max(name.len());

        buf.clear();
        let _ = write!(buf, "{}", value);
        value_len = value_len.max(buf.len());
    }

    Lengths {
        idx: idx_len,
        name: name_len,
        value: value_len,
    }
}

struct Lengths {
    idx: usize,
    name: usize,
    value: usize,
}
