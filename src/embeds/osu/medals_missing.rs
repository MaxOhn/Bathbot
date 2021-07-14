use crate::{
    commands::osu::MedalType,
    embeds::{Author, Footer},
    util::{constants::OSU_BASE, CowUtils},
};

use rosu_v2::model::user::User;
use std::fmt::Write;

pub struct MedalsMissingEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
    title: &'static str,
}

impl MedalsMissingEmbed {
    pub fn new(
        user: &User,
        medals: &[MedalType],
        medal_count: (usize, usize),
        includes_last: bool,
        pages: (usize, usize),
    ) -> Self {
        let mut description = String::new();

        for (i, medal) in medals.iter().enumerate() {
            match medal {
                MedalType::Group(g) => {
                    let _ = writeln!(description, "__**{}:**__", g);

                    if let Some(MedalType::Group(_)) = medals.get(i + 1) {
                        description.push_str("All medals acquired\n");
                    } else if i == medals.len() - 1 && includes_last {
                        description.push_str("All medals acquired");
                    }
                }
                MedalType::Medal(m) => {
                    let _ = writeln!(
                        description,
                        "- [{}](https://osekai.net/medals/?medal={})",
                        m.name,
                        m.name.cow_replace(' ', "+")
                    );
                }
            }
        }

        let footer = Footer::new(format!(
            "Page {}/{} | Missing {}/{} medals",
            pages.0, pages.1, medal_count.0, medal_count.1
        ));

        let author = Author::new(&user.username)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, &user.country_code
            ));

        Self {
            author,
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
            title: "Missing medals",
        }
    }
}

impl_builder!(MedalsMissingEmbed {
    author,
    description,
    footer,
    thumbnail,
    title,
});
