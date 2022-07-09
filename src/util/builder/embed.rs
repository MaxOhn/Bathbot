use time::OffsetDateTime;
use twilight_model::{
    channel::embed::{Embed, EmbedAuthor, EmbedField, EmbedImage, EmbedThumbnail},
    util::Timestamp,
};

use crate::util::constants::DARK_GREEN;

use super::footer::IntoEmbedFooter;

#[derive(Clone)]
pub struct EmbedBuilder(Embed);

impl Default for EmbedBuilder {
    fn default() -> Self {
        Self(Embed {
            author: None,
            color: Some(DARK_GREEN),
            description: None,
            fields: Vec::new(),
            footer: None,
            image: None,
            kind: String::new(),
            provider: None,
            thumbnail: None,
            timestamp: None,
            title: None,
            url: None,
            video: None,
        })
    }
}

impl EmbedBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(mut self) -> Embed {
        self.0.kind.push_str("rich");

        self.0
    }

    pub fn author(mut self, author: impl Into<EmbedAuthor>) -> Self {
        self.0.author = Some(author.into());

        self
    }

    pub fn color(mut self, color: u32) -> Self {
        self.0.color = Some(color);

        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.0.description = Some(description);

        self
    }

    pub fn fields(mut self, fields: Vec<EmbedField>) -> Self {
        self.0.fields = fields;

        self
    }

    pub fn footer(mut self, footer: impl IntoEmbedFooter) -> Self {
        self.0.footer = Some(footer.into());

        self
    }

    pub fn image(mut self, image: impl Into<String>) -> Self {
        let url = image.into();

        if !url.is_empty() {
            let image = EmbedImage {
                height: None,
                width: None,
                proxy_url: None,
                url,
            };

            self.0.image = Some(image);
        }

        self
    }

    pub fn timestamp(mut self, timestamp: OffsetDateTime) -> Self {
        self.0.timestamp = Timestamp::from_secs(timestamp.unix_timestamp() as i64).ok();

        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = Some(title.into());

        self
    }

    pub fn thumbnail(mut self, thumbnail: impl Into<String>) -> Self {
        let url = thumbnail.into();

        if !url.is_empty() {
            let thumbnail = EmbedThumbnail {
                height: None,
                width: None,
                proxy_url: None,
                url,
            };

            self.0.thumbnail = Some(thumbnail);
        }

        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.0.url = Some(url.into());

        self
    }
}
