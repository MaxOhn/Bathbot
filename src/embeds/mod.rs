mod fun;
mod osu;
mod owner;
mod twitch;
mod utility;

pub use fun::*;
pub use osu::*;
pub use owner::*;
pub use twitch::*;
pub use utility::*;

use crate::util::{constants::DARK_GREEN, datetime};

use chrono::{DateTime, Utc};
use twilight::builders::embed::EmbedBuilder;

pub trait EmbedData: Send + Sync + Sized + Clone {
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
    fn image(&self) -> Option<&str> {
        None
    }
    fn thumbnail(&self) -> Option<&str> {
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

    // Implement this if minimization required
    fn minimize(&self) -> EmbedBuilder {
        EmbedBuilder::new()
    }

    // Don't implement this
    fn build(&self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();
        if let Some(title) = self.title() {
            eb = eb.title(title);
        }
        if let Some(url) = self.url() {
            eb = eb.url(url);
        }
        if let Some(timestamp) = self.timestamp() {
            let timestamp = datetime::date_to_string(timestamp);
            eb = eb.timestamp(timestamp);
        }
        if let Some(thumbnail) = self.thumbnail() {
            eb = eb.thumbnail(thumbnail);
        }
        if let Some(image) = self.image() {
            eb = eb.image(image);
        }
        if let Some(footer) = self.footer() {
            let mut fb = eb.footer(&footer.text);
            if let Some(ref icon_url) = footer.icon_url {
                fb = fb.icon_url(icon_url);
            }
            eb = fb.commit();
        }
        if let Some(author) = self.author() {
            let mut ab = eb.author().name(&author.name);
            if let Some(ref icon_url) = author.icon_url {
                ab = ab.icon_url(icon_url);
            }
            if let Some(ref url) = author.url {
                ab = ab.url(url);
            }
            eb = ab.commit();
        }
        if let Some(description) = self.description() {
            eb = eb.description(description);
        }
        if let Some(fields) = self.fields() {
            for (name, value, inline) in fields {
                let field = eb.add_field(name, value);
                eb = if inline {
                    field.inline().commit()
                } else {
                    field.commit()
                }
            }
        }
        eb.color(DARK_GREEN)
    }
}

#[derive(Clone)]
pub struct Footer {
    text: String,
    icon_url: Option<String>,
}

impl Footer {
    pub fn new(text: String) -> Self {
        Self {
            text,
            icon_url: None,
        }
    }
    pub fn icon_url(mut self, icon_url: String) -> Self {
        self.icon_url = Some(icon_url);
        self
    }
}

#[derive(Clone)]
pub struct Author {
    name: String,
    url: Option<String>,
    icon_url: Option<String>,
}

impl Author {
    pub fn new(name: String) -> Self {
        Self {
            name,
            url: None,
            icon_url: None,
        }
    }
    pub fn url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }
    pub fn icon_url(mut self, icon_url: String) -> Self {
        self.icon_url = Some(icon_url);
        self
    }
}
