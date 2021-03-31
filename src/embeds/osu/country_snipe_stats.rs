use crate::{
    custom_client::SnipeCountryStatistics,
    embeds::{attachment, EmbedFields, Footer},
    util::{
        constants::OSU_BASE,
        numbers::{round, with_comma_uint},
        Country,
    },
};

pub struct CountrySnipeStatsEmbed {
    thumbnail: String,
    title: String,
    footer: Footer,
    image: String,
    fields: EmbedFields,
}

impl CountrySnipeStatsEmbed {
    pub fn new(country: Option<&Country>, statistics: SnipeCountryStatistics) -> Self {
        let mut fields = EmbedFields::with_capacity(2);

        if let Some(top_gain) = statistics.top_gain {
            fields.push(field!(
                "Most gained",
                format!("{} ({:+})", top_gain.username, top_gain.difference),
                true
            ));
        }

        if let Some(top_loss) = statistics.top_loss {
            fields.push(field!(
                "Most losses",
                format!("{} ({:+})", top_loss.username, top_loss.difference),
                true
            ));
        }

        let percent = round(100.0 * statistics.unplayed_maps as f32 / statistics.total_maps as f32);

        let (title, thumbnail) = match country {
            Some(country) => {
                let title = format!(
                    "{}{} #1 statistics",
                    country.name,
                    if country.name.ends_with('s') {
                        "'"
                    } else {
                        "'s"
                    }
                );

                let thumbnail = format!("{}/images/flags/{}.png", OSU_BASE, country.acronym);

                (title, thumbnail)
            }
            None => ("Global #1 statistics".to_owned(), String::new()),
        };

        let footer = Footer::new(format!(
            "Unplayed maps: {}/{} ({}%)",
            with_comma_uint(statistics.unplayed_maps),
            with_comma_uint(statistics.total_maps),
            percent
        ));

        Self {
            fields,
            thumbnail,
            title,
            footer,
            image: attachment("stats_graph.png"),
        }
    }
}

impl_builder!(CountrySnipeStatsEmbed {
    fields,
    footer,
    image,
    thumbnail,
    title,
});
