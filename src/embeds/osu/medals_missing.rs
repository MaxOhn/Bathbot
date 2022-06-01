use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::model::user::User;

use crate::{
    commands::osu::MedalType,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        osu::flag_url,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct MedalsMissingEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MedalsMissingEmbed {
    pub fn new(
        user: &User,
        medals: &[MedalType],
        medal_count: (usize, usize),
        includes_last: bool,
        pages: &Pages,
    ) -> Self {
        let mut description = String::new();

        for (i, medal) in medals.iter().enumerate() {
            match medal {
                MedalType::Group(g) => {
                    let _ = writeln!(description, "__**{g}:**__");

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

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} | Missing {}/{} medals",
            medal_count.0, medal_count.1
        ));

        let author = AuthorBuilder::new(user.username.as_str())
            .url(format!("{OSU_BASE}u/{}", user.user_id))
            .icon_url(flag_url(user.country_code.as_str()));

        Self {
            author,
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
            title: "Missing medals",
        }
    }
}
