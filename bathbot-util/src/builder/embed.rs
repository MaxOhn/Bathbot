use std::num::NonZeroU32;

use time::OffsetDateTime;
use twilight_model::{
    channel::message::embed::{Embed, EmbedField, EmbedImage, EmbedThumbnail},
    util::Timestamp,
};

use super::footer::IntoFooterBuilder;
use crate::{
    AuthorBuilder, FooterBuilder,
    constants::{DARK_GREEN, RED},
};

#[derive(Clone, Default)]
pub struct EmbedBuilder {
    pub author: Option<AuthorBuilder>,
    pub color: Option<NonZeroU32>,
    pub description: Option<String>,
    pub fields: Vec<EmbedField>,
    pub footer: Option<FooterBuilder>,
    pub image_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub timestamp: Option<Timestamp>,
    pub title: Option<String>,
    pub url: Option<String>,
}

impl EmbedBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Embed {
        Embed {
            author: self.author.map(AuthorBuilder::build),
            color: Some(self.color.map_or(DARK_GREEN, NonZeroU32::get)),
            description: self.description,
            fields: self.fields,
            footer: self.footer.map(FooterBuilder::build),
            image: self.image_url.map(|url| EmbedImage {
                height: None,
                proxy_url: None,
                url,
                width: None,
            }),
            kind: "rich".to_owned(),
            provider: None,
            thumbnail: self.thumbnail_url.map(|url| EmbedThumbnail {
                height: None,
                proxy_url: None,
                url,
                width: None,
            }),
            timestamp: self.timestamp,
            title: self.title,
            url: self.url,
            video: None,
        }
    }

    pub fn author(mut self, author: AuthorBuilder) -> Self {
        self.author = Some(author);

        self
    }

    pub fn color_green(self) -> Self {
        self.color(DARK_GREEN)
    }

    pub fn color_red(self) -> Self {
        self.color(RED)
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn color(mut self, color: u32) -> Self {
        debug_assert!(color != 0, "color {color} must be non-zero");

        // SAFETY: This method is private and only used for the RED and DARK_GREEN
        // constants which are both non-zero but even if they were zero, it would have
        // been caught with the debug_assert.
        self.color = Some(unsafe { NonZeroU32::new_unchecked(color) });

        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);

        self
    }

    pub fn fields(mut self, fields: Vec<EmbedField>) -> Self {
        self.fields = fields;

        self
    }

    pub fn push_field(&mut self, field: EmbedField) {
        self.fields.push(field);
    }

    pub fn footer(mut self, footer: impl IntoFooterBuilder) -> Self {
        self.footer = Some(footer.into());

        self
    }

    pub fn image(mut self, url: impl Into<String>) -> Self {
        let url = url.into();

        if !url.is_empty() {
            self.image_url = Some(url);
        }

        self
    }

    pub fn thumbnail(mut self, url: impl Into<String>) -> Self {
        let url = url.into();

        if !url.is_empty() {
            self.thumbnail_url = Some(url);
        }

        self
    }

    pub fn timestamp(mut self, timestamp: OffsetDateTime) -> Self {
        self.timestamp = Timestamp::from_secs(timestamp.unix_timestamp()).ok();

        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());

        self
    }
}
