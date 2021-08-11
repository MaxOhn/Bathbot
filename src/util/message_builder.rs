use crate::embeds::EmbedBuilder;

use std::borrow::Cow;
use twilight_model::channel::embed::Embed;

#[derive(Default)]
pub struct MessageBuilder<'c> {
    pub content: Option<Cow<'c, str>>,
    pub embed: Option<Embed>,
    pub file: Option<(&'static str, &'c [u8])>,
}

impl<'c> MessageBuilder<'c> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<Cow<'c, str>>) -> Self {
        self.content.replace(content.into());

        self
    }

    pub fn embed(mut self, embed: impl IntoEmbed) -> Self {
        self.embed.replace(embed.into_embed());

        self
    }

    pub fn file(mut self, name: &'static str, data: &'c [u8]) -> Self {
        self.file.replace((name, data));

        self
    }
}

impl<'c> From<Embed> for MessageBuilder<'c> {
    fn from(embed: Embed) -> Self {
        Self {
            content: None,
            embed: Some(embed),
            file: None,
        }
    }
}

pub trait IntoEmbed {
    fn into_embed(self) -> Embed;
}

impl IntoEmbed for Embed {
    fn into_embed(self) -> Embed {
        self
    }
}

impl IntoEmbed for String {
    fn into_embed(self) -> Embed {
        EmbedBuilder::new().description(self).build()
    }
}

impl<'s> IntoEmbed for &'s str {
    fn into_embed(self) -> Embed {
        EmbedBuilder::new().description(self).build()
    }
}
