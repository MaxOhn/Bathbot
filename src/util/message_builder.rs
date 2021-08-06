use std::borrow::Cow;
use twilight_model::channel::embed::Embed;

#[derive(Default)]
pub struct MessageBuilder<'c> {
    pub content: Option<Cow<'c, str>>,
    pub embed: Option<Embed>,
}

impl<'c> MessageBuilder<'c> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<Cow<'c, str>>) -> Self {
        self.content.replace(content.into());

        self
    }

    pub fn embed(mut self, embed: Embed) -> Self {
        self.embed.replace(embed);

        self
    }
}
