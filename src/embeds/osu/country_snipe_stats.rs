use crate::{
    custom_client::SnipeCountryStatistics,
    embeds::{EmbedData, EmbedFields, Footer},
    util::{
        constants::OSU_BASE,
        numbers::{round, with_comma_uint},
        Country,
    },
};

use twilight_embed_builder::image_source::ImageSource;

pub struct CountrySnipeStatsEmbed {
    thumbnail: Option<ImageSource>,
    title: Option<String>,
    footer: Option<Footer>,
    image: Option<ImageSource>,
    fields: EmbedFields,
}

impl CountrySnipeStatsEmbed {
    pub fn new(country: Option<&Country>, statistics: SnipeCountryStatistics) -> Self {
        let mut fields = EmbedFields::with_capacity(2);

        if let Some(top_gain) = statistics.top_gain {
            fields.push((
                String::from("Most gained"),
                format!("{} ({:+})", top_gain.username, top_gain.difference),
                true,
            ));
        }

        if let Some(top_loss) = statistics.top_loss {
            fields.push((
                String::from("Most losses"),
                format!("{} ({:+})", top_loss.username, top_loss.difference),
                true,
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

                let thumbnail =
                    ImageSource::url(format!("{}/images/flags/{}.png", OSU_BASE, country.acronym))
                        .ok();

                (title, thumbnail)
            }
            None => (String::from("Global #1 statistics"), None),
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
            title: Some(title),
            footer: Some(footer),
            image: Some(ImageSource::attachment("stats_graph.png").unwrap()),
        }
    }
}

impl EmbedData for CountrySnipeStatsEmbed {
    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }

    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }

    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }

    fn image_owned(&mut self) -> Option<ImageSource> {
        self.image.take()
    }

    fn fields_owned(self) -> Option<EmbedFields> {
        Some(self.fields)
    }
}
