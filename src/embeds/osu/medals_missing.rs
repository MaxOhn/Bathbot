use crate::{
    commands::osu::MedalType,
    custom_client::OsuProfile,
    embeds::{Author, EmbedData, Footer},
    util::constants::{AVATAR_URL, OSU_BASE},
};

use cow_utils::CowUtils;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct MedalsMissingEmbed {
    thumbnail: ImageSource,
    author: Author,
    description: String,
    footer: Footer,
}

impl MedalsMissingEmbed {
    pub fn new(
        profile: &OsuProfile,
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

        let author = Author::new(&profile.username)
            .url(format!("{}u/{}", OSU_BASE, profile.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, &profile.country_code
            ));

        Self {
            footer,
            author,
            description,
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, profile.user_id)).unwrap(),
        }
    }
}

impl EmbedData for MedalsMissingEmbed {
    fn title(&self) -> Option<&str> {
        Some("Missing medals")
    }

    fn description(&self) -> Option<&str> {
        Some(self.description.as_str())
    }

    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
