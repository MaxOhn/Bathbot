mod basic_embed;
mod fun;
mod osu;
mod recent;
mod simulate;
mod streams;
mod util;
mod utility;

pub use fun::*;
pub use osu::*;
pub use streams::*;
pub use utility::*;

pub use basic_embed::BasicEmbedData;
pub use recent::RecentData;
pub use simulate::SimulateData;

use chrono::{DateTime, Utc};
use serenity::{builder::CreateEmbed, utils::Colour};

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
    // fn minimize<'e>(&self, e: &' mut CreateEmbed) -> &'e mut CreateEmbed;
    fn minimize<'e>(&self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        embed
    }

    // Don't implement this
    fn build<'e>(&self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        if let Some(title) = self.title() {
            e.title(title);
        }
        if let Some(url) = self.url() {
            e.url(url);
        }
        if let Some(timestamp) = self.timestamp() {
            e.timestamp(timestamp);
        }
        if let Some(thumbnail) = self.thumbnail() {
            e.thumbnail(thumbnail);
        }
        if let Some(image) = self.image() {
            e.image(image);
        }
        if let Some(footer) = self.footer() {
            e.footer(|f| {
                if let Some(ref icon_url) = footer.icon_url {
                    f.icon_url(icon_url);
                }
                f.text(&footer.text)
            });
        }
        if let Some(author) = self.author() {
            e.author(|a| {
                if let Some(ref icon_url) = author.icon_url {
                    a.icon_url(icon_url);
                }
                if let Some(ref url) = author.url {
                    a.url(url);
                }
                a.name(&author.name)
            });
        }
        if let Some(description) = self.description() {
            e.description(description);
        }
        if let Some(fields) = self.fields() {
            e.fields(fields);
        }
        e.color(Colour::DARK_GREEN)
    }
}

impl EmbedData for BasicEmbedData {}

impl EmbedData for RecentData {
    fn build<'e>(&self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        self.build_embed(e)
    }
    fn minimize<'e>(&self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        self.minimize(embed)
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
