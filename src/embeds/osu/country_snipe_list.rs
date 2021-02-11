use crate::{
    commands::osu::SnipeOrder,
    custom_client::SnipeCountryPlayer,
    embeds::{EmbedData, Footer},
    util::{
        constants::OSU_BASE,
        numbers::{with_comma, with_comma_u64},
        Country,
    },
};

use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct CountrySnipeListEmbed {
    thumbnail: Option<ImageSource>,
    description: String,
    title: String,
    footer: Footer,
}

impl CountrySnipeListEmbed {
    pub fn new<'i, S>(
        country: Option<&Country>,
        order: SnipeOrder,
        players: S,
        author_idx: Option<usize>,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, SnipeCountryPlayer)>,
    {
        let order_text = match order {
            SnipeOrder::Count => "#1 count",
            SnipeOrder::PP => "average pp of #1s",
            SnipeOrder::Stars => "average stars of #1s",
            SnipeOrder::WeightedPP => "weighted pp from #1s",
        };

        let (title, thumbnail) = match country {
            Some(country) => {
                let title = format!(
                    "{}{} #1 list, sorted by {}",
                    country.name,
                    if country.name.ends_with('s') {
                        "'"
                    } else {
                        "'s"
                    },
                    order_text
                );

                let thumbnail =
                    ImageSource::url(format!("{}/images/flags/{}.png", OSU_BASE, country.acronym))
                        .ok();

                (title, thumbnail)
            }
            None => (
                format!("Global #1 statistics, sorted by {}", order_text),
                None,
            ),
        };

        let mut description = String::with_capacity(512);

        for (idx, player) in players {
            let _ = writeln!(
                description,
                "**{idx}. [{name}]({base}users/{id})**: {w}Weighted pp: {weighted}{w}\n\
                {c}Count: {count}{c} ~ {p}Avg pp: {pp}{p} ~ {s}Avg stars: {stars:.2}★{s}",
                idx = idx,
                name = player.username,
                base = OSU_BASE,
                id = player.user_id,
                c = if order == SnipeOrder::Count { "__" } else { "" },
                p = if order == SnipeOrder::PP { "__" } else { "" },
                s = if order == SnipeOrder::Stars { "__" } else { "" },
                w = if order == SnipeOrder::WeightedPP {
                    "__"
                } else {
                    ""
                },
                count = with_comma_u64(player.count_first as u64),
                pp = with_comma(player.avg_pp),
                stars = player.avg_sr,
                weighted = with_comma(player.pp),
            );
        }

        description.pop();
        let mut footer_text = format!("Page {}/{}", pages.0, pages.1);

        if let Some(idx) = author_idx {
            let _ = write!(footer_text, " ~ Your position: {}", idx + 1);
        }

        Self {
            description,
            title,
            thumbnail,
            footer: Footer::new(footer_text),
        }
    }
}

impl EmbedData for CountrySnipeListEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }

    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        self.thumbnail.as_ref()
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
