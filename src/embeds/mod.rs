mod fun;
mod osu;
mod owner;
mod tracking;
mod twitch;
mod utility;

pub use fun::*;
pub use osu::*;
pub use owner::*;
pub use tracking::*;
pub use twitch::*;
pub use utility::*;

use crate::{
    unwind_error,
    util::{constants::DARK_GREEN, datetime},
};

use chrono::{DateTime, Utc};
use twilight_embed_builder::{
    author::EmbedAuthorBuilder, builder::EmbedBuilder, footer::EmbedFooterBuilder,
    image_source::ImageSource,
};
use twilight_model::channel::embed::EmbedField;

pub trait EmbedData: Send + Sync + Sized {
    // Make these point to the corresponding fields
    fn title(&self) -> Option<&str> {
        None
    }
    fn url(&self) -> Option<&str> {
        None
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        None
    }
    fn image(&self) -> Option<&ImageSource> {
        None
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        None
    }
    fn footer(&self) -> Option<&Footer> {
        None
    }
    fn author(&self) -> Option<&Author> {
        None
    }
    fn description(&self) -> Option<&str> {
        None
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        None
    }
    // ---
    fn title_owned(&mut self) -> Option<String> {
        None
    }
    fn url_owned(&mut self) -> Option<String> {
        None
    }
    fn image_owned(&mut self) -> Option<ImageSource> {
        None
    }
    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        None
    }
    fn footer_owned(&mut self) -> Option<Footer> {
        None
    }
    fn author_owned(&mut self) -> Option<Author> {
        None
    }
    fn description_owned(&mut self) -> Option<String> {
        None
    }
    fn fields_owned(self) -> Option<Vec<(String, String, bool)>> {
        None
    }

    // Implement this if minimization required
    fn minimize(self) -> EmbedBuilder {
        EmbedBuilder::new()
    }

    // Don't implement this
    fn build(&self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();
        if let Some(title) = self.title() {
            eb = eb.title(title).unwrap();
        }
        if let Some(url) = self.url() {
            eb = eb.url(url);
        }
        if let Some(timestamp) = self.timestamp() {
            let timestamp = datetime::date_to_string(timestamp);
            eb = eb.timestamp(timestamp);
        }
        if let Some(thumbnail) = self.thumbnail() {
            eb = eb.thumbnail(thumbnail.to_owned());
        }
        if let Some(image) = self.image() {
            eb = eb.image(image.to_owned());
        }
        if let Some(footer) = self.footer() {
            match EmbedFooterBuilder::new(&footer.text) {
                Ok(mut fb) => {
                    if let Some(ref icon_url) = footer.icon_url {
                        fb = fb.icon_url(icon_url.to_owned());
                    }
                    eb = eb.footer(fb);
                }
                Err(why) => unwind_error!(warn, why, "Invalid footer text `{}`: {}", footer.text),
            }
        }
        if let Some(author) = self.author() {
            match EmbedAuthorBuilder::new().name(&author.name) {
                Ok(mut ab) => {
                    if let Some(ref icon_url) = author.icon_url {
                        ab = ab.icon_url(icon_url.to_owned());
                    }
                    if let Some(ref url) = author.url {
                        ab = ab.url(url);
                    }
                    eb = eb.author(ab);
                }
                Err(why) => unwind_error!(warn, why, "Invalid author name `{}`: {}", author.name),
            }
        }
        if let Some(description) = self.description().filter(|d| !d.is_empty()) {
            eb = eb.description(description).unwrap();
        }
        if let Some(fields) = self.fields() {
            for (name, value, inline) in fields {
                eb = eb.field(EmbedField {
                    name,
                    value,
                    inline,
                });
            }
        }
        eb.color(DARK_GREEN).unwrap()
    }

    fn build_owned(mut self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();
        if let Some(title) = self.title_owned() {
            eb = eb.title(title).unwrap();
        }
        if let Some(url) = self.url_owned() {
            eb = eb.url(url);
        }
        if let Some(timestamp) = self.timestamp() {
            let timestamp = datetime::date_to_string(timestamp);
            eb = eb.timestamp(timestamp);
        }
        if let Some(thumbnail) = self.thumbnail_owned() {
            eb = eb.thumbnail(thumbnail);
        }
        if let Some(image) = self.image_owned() {
            eb = eb.image(image);
        }
        if let Some(mut footer) = self.footer_owned() {
            match EmbedFooterBuilder::new(footer.text) {
                Ok(mut fb) => {
                    if let Some(icon_url) = footer.icon_url.take() {
                        fb = fb.icon_url(icon_url);
                    }
                    eb = eb.footer(fb);
                }
                Err(why) => unwind_error!(warn, why, "Invalid footer text: {}"),
            }
        }
        if let Some(mut author) = self.author_owned() {
            match EmbedAuthorBuilder::new().name(author.name) {
                Ok(mut ab) => {
                    if let Some(icon_url) = author.icon_url.take() {
                        ab = ab.icon_url(icon_url);
                    }
                    if let Some(url) = author.url.take() {
                        ab = ab.url(url);
                    }
                    eb = eb.author(ab);
                }
                Err(why) => unwind_error!(warn, why, "Invalid author name: {}"),
            }
        }
        if let Some(description) = self.description_owned().filter(|d| !d.is_empty()) {
            eb = eb.description(description).unwrap();
        }
        if let Some(fields) = self.fields_owned() {
            for (name, value, inline) in fields {
                eb = eb.field(EmbedField {
                    name,
                    value,
                    inline,
                });
            }
        }
        eb.color(DARK_GREEN).unwrap()
    }
}

#[derive(Clone)]
pub struct Footer {
    text: String,
    icon_url: Option<ImageSource>,
}

impl Footer {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon_url: None,
        }
    }
    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        self.icon_url = Some(ImageSource::url(icon_url).unwrap());
        self
    }
}

#[derive(Clone)]
pub struct Author {
    name: String,
    url: Option<String>,
    icon_url: Option<ImageSource>,
}

impl Author {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            icon_url: None,
        }
    }
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        self.icon_url = Some(ImageSource::url(icon_url).unwrap());
        self
    }
}
