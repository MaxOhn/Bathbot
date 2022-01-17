use crate::{
    commands::osu::SnipeOrder,
    custom_client::SnipeCountryPlayer,
    embeds::Footer,
    util::{
        constants::OSU_BASE,
        numbers::{with_comma_float, with_comma_int},
        osu::flag_url,
        CountryCode,
    },
};

use std::fmt::Write;

pub struct CountrySnipeListEmbed {
    thumbnail: String,
    description: String,
    title: String,
    footer: Footer,
}

impl CountrySnipeListEmbed {
    pub fn new<'i, S>(
        country: Option<&(String, CountryCode)>,
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
            SnipeOrder::Pp => "average pp of #1s",
            SnipeOrder::Stars => "average stars of #1s",
            SnipeOrder::WeightedPp => "weighted pp from #1s",
        };

        let (title, thumbnail) = match country {
            Some((country, code)) => {
                let title = format!(
                    "{country}{} #1 list, sorted by {order_text}",
                    if country.ends_with('s') { "'" } else { "'s" },
                );

                let thumbnail = flag_url(code.as_str());

                (title, thumbnail)
            }
            None => (
                format!("Global #1 statistics, sorted by {order_text}"),
                String::new(),
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
                p = if order == SnipeOrder::Pp { "__" } else { "" },
                s = if order == SnipeOrder::Stars { "__" } else { "" },
                w = if order == SnipeOrder::WeightedPp {
                    "__"
                } else {
                    ""
                },
                count = with_comma_int(player.count_first),
                pp = with_comma_float(player.avg_pp),
                stars = player.avg_sr,
                weighted = with_comma_float(player.pp),
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

impl_builder!(CountrySnipeListEmbed {
    description,
    footer,
    thumbnail,
    title,
});
