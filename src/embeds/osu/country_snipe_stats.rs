use crate::{
    custom_client::SnipeTopDifference,
    embeds::EmbedData,
    util::{constants::OSU_BASE, numbers::with_comma_u64, Country},
};

use twilight_embed_builder::image_source::ImageSource;

pub struct CountrySnipeStatsEmbed {
    thumbnail: Option<ImageSource>,
    title: String,
    image: ImageSource,
    fields: Vec<(String, String, bool)>,
}

impl CountrySnipeStatsEmbed {
    pub fn new(
        country: Option<&Country>,
        differences: Option<(SnipeTopDifference, SnipeTopDifference)>,
        unplayed: u64,
    ) -> Self {
        let mut fields = if let Some((gain, loss)) = differences {
            vec![
                (
                    String::from("Most gained"),
                    format!("{} ({:+})", gain.name, gain.difference),
                    true,
                ),
                (
                    String::from("Most losses"),
                    format!("{} ({:+})", loss.name, loss.difference),
                    true,
                ),
            ]
        } else {
            Vec::with_capacity(1)
        };
        fields.push((
            String::from("Unplayed maps"),
            with_comma_u64(unplayed),
            true,
        ));
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
        Self {
            fields,
            title,
            thumbnail,
            image: ImageSource::attachment("stats_graph.png").unwrap(),
        }
    }
}

impl EmbedData for CountrySnipeStatsEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        self.thumbnail.as_ref()
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
