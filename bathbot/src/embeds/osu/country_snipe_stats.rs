use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::{CountryName, SnipeCountryStatistics};
use bathbot_util::{
    CowUtils, FooterBuilder, attachment, fields,
    numbers::{WithComma, round},
    osu::flag_url,
};
use rosu_v2::prelude::CountryCode;
use twilight_model::channel::message::embed::EmbedField;

#[derive(EmbedData)]
pub struct CountrySnipeStatsEmbed {
    thumbnail: String,
    title: String,
    footer: FooterBuilder,
    image: String,
    fields: Vec<EmbedField>,
}

impl CountrySnipeStatsEmbed {
    pub fn new(country: Option<(CountryName, CountryCode)>, stats: SnipeCountryStatistics) -> Self {
        let mut fields = Vec::with_capacity(2);

        let gains_value = if let (Some(ref username), Some(count)) =
            (stats.most_gains_username, stats.most_gains_count)
        {
            format!("{} ({count:+})", username.cow_escape_markdown())
        } else {
            "Unknown".to_owned()
        };

        fields![fields { "Most gained", gains_value, true }];

        let losses_value = if let (Some(ref username), Some(count)) =
            (stats.most_losses_username, stats.most_losses_count)
        {
            format!("{} ({count:+})", username.cow_escape_markdown())
        } else {
            "Unknown".to_owned()
        };

        fields![fields { "Most losses", losses_value, true }];

        let (title, thumbnail) = match country {
            Some((country, code)) => {
                let title = format!(
                    "{country}{} #1 statistics",
                    if country.ends_with('s') { "'" } else { "'s" }
                );

                let thumbnail = flag_url(code.as_str());

                (title, thumbnail)
            }
            None => ("Global #1 statistics".to_owned(), String::new()),
        };

        let mut footer_text = format!(
            "Unplayed maps: {unplayed}",
            unplayed = WithComma::new(stats.unplayed_maps),
        );

        if let Some(total_maps) = stats.total_maps {
            let _ = write!(
                footer_text,
                "/{total} ({percent}%)",
                total = WithComma::new(total_maps),
                percent = round(100.0 * stats.unplayed_maps as f32 / total_maps as f32)
            );
        }

        let footer = FooterBuilder::new(footer_text);

        Self {
            fields,
            thumbnail,
            title,
            footer,
            image: attachment("stats_graph.png"),
        }
    }
}
