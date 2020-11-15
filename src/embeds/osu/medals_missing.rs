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

/*
"__**Skill:**__
- [Approaching the summit](https://osekai.net/medals/?medal=Approaching+the+summit)
- [Aberration](https://osekai.net/medals/?medal=Aberration)
- [Maniac](https://osekai.net/medals/?medal=Maniac)
- [Quickening](https://osekai.net/medals/?medal=Quickening)
- [Supersonic](https://osekai.net/medals/?medal=Supersonic)
- [Dashing Scarlet](https://osekai.net/medals/?medal=Dashing+Scarlet)
- [Level Breaker](https://osekai.net/medals/?medal=Level+Breaker)
- [Step Up](https://osekai.net/medals/?medal=Step+Up)
- [Behind The Veil](https://osekai.net/medals/?medal=Behind+The+Veil)
- [Chosen](https://osekai.net/medals/?medal=Chosen)
- [Phantasm](https://osekai.net/medals/?medal=Phantasm)
- [Unfathomable](https://osekai.net/medals/?medal=Unfathomable)
__**Dedication:**__
- [3,000,000 Drum Hits](https://osekai.net/medals/?medal=3,000,000+Drum+Hits)
__**Hush-Hush:**__
- [Skylord](https://osekai.net/medals/?medal=Skylord)
- [Not Bluffing](https://osekai.net/medals/?medal=Not+Bluffing)
__**Beatmap Packs:**__
- [Camellia I](https://osekai.net/medals/?medal=Camellia+I)
- [Camellia II](https://osekai.net/medals/?medal=Camellia+II)
- [Celldweller](https://osekai.net/medals/?medal=Celldweller)
- [Cranky II](https://osekai.net/medals/?medal=Cranky+II)
- [Cute Anime Girls](https://osekai.net/medals/?medal=Cute+Anime+Girls)
- [ELFENSJoN](https://osekai.net/medals/?medal=ELFENSJoN)
- [Hyper Potions](https://osekai.net/medals/?medal=Hyper+Potions)
- [Kola Kid](https://osekai.net/medals/?medal=Kola+Kid)
- [LeaF](https://osekai.net/medals/?medal=LeaF)
- [Panda Eyes](https://osekai.net/medals/?medal=Panda+Eyes)
- [PUP](https://osekai.net/medals/?medal=PUP)
- [Ricky Montgomery](https://osekai.net/medals/?medal=Ricky+Montgomery)
- [Rin](https://osekai.net/medals/?medal=Rin)
- [S3RL](https://osekai.net/medals/?medal=S3RL)
- [Sound Souler](https://osekai.net/medals/?medal=Sound+Souler)
- [Teminite](https://osekai.net/medals/?medal=Teminite)
- [VINXIS](https://osekai.net/medals/?medal=VINXIS)
__**Seasonal Spotlights:**__
All medals acquired
__**Beatmap Spotlights:**__
All medals acquired"
*/
