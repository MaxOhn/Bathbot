use std::{borrow::Cow, slice};

use twilight_model::{
    channel::message::{embed::Embed, Component},
    http::attachment::Attachment,
};

use super::EmbedBuilder;

#[derive(Default)]
pub struct MessageBuilder<'c> {
    pub content: Option<Cow<'c, str>>,
    pub embed: EmbedOption,
    pub attachment: Option<Attachment>,
    pub components: Option<Vec<Component>>,
}

// essentially an extension to Option<EmbedBuilder> which will be Some most of
// the time
#[allow(clippy::large_enum_variant)]
#[derive(Default)]
pub enum EmbedOption {
    Include(EmbedBuilder),
    Clear,
    #[default]
    None,
}

impl EmbedOption {
    pub fn build(self) -> BuiltEmbedOption {
        match self {
            EmbedOption::Include(embed) => BuiltEmbedOption::Include(embed.build()),
            EmbedOption::Clear => BuiltEmbedOption::Clear,
            EmbedOption::None => BuiltEmbedOption::None,
        }
    }
}

impl From<EmbedOption> for Option<Vec<Embed>> {
    fn from(embed: EmbedOption) -> Self {
        match embed {
            EmbedOption::Include(embed) => Some(vec![embed.build()]),
            EmbedOption::Clear => Some(Vec::new()),
            EmbedOption::None => None,
        }
    }
}

// essentially an extension to Option<Embed> which will be Some most of the time
#[allow(clippy::large_enum_variant)]
pub enum BuiltEmbedOption {
    Include(Embed),
    Clear,
    None,
}

impl BuiltEmbedOption {
    pub fn as_option_slice(&self) -> Option<&'_ [Embed]> {
        match self {
            BuiltEmbedOption::Include(embed) => Some(slice::from_ref(embed)),
            BuiltEmbedOption::Clear => Some(&[]),
            BuiltEmbedOption::None => None,
        }
    }
}

impl<'c> MessageBuilder<'c> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content(mut self, content: impl Into<Cow<'c, str>>) -> Self {
        self.content = Some(content.into());

        self
    }

    pub fn embed(mut self, embed: impl IntoEmbed) -> Self {
        self.embed = embed.into_embed();

        self
    }

    pub fn attachment(mut self, name: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.attachment = Some(Attachment::from_bytes(name.into(), bytes, 1));

        self
    }

    pub fn components(mut self, components: Vec<Component>) -> Self {
        self.components = Some(components);

        self
    }
}

impl From<EmbedBuilder> for MessageBuilder<'_> {
    #[inline]
    fn from(embed: EmbedBuilder) -> Self {
        Self {
            embed: EmbedOption::Include(embed),
            ..Default::default()
        }
    }
}

/// Not implementing this for [`Embed`] itself because turning [`Embed`] into
/// [`EmbedBuilder`] is not efficient so it should be avoided.
pub trait IntoEmbed {
    fn into_embed(self) -> EmbedOption;
}

impl IntoEmbed for EmbedBuilder {
    #[inline]
    fn into_embed(self) -> EmbedOption {
        EmbedOption::Include(self)
    }
}

impl IntoEmbed for String {
    #[inline]
    fn into_embed(self) -> EmbedOption {
        EmbedOption::Include(EmbedBuilder::new().description(self))
    }
}

impl<'s> IntoEmbed for &'s str {
    #[inline]
    fn into_embed(self) -> EmbedOption {
        EmbedOption::Include(EmbedBuilder::new().description(self))
    }
}

impl IntoEmbed for Option<EmbedBuilder> {
    #[inline]
    fn into_embed(self) -> EmbedOption {
        match self {
            Some(inner) => EmbedOption::Include(inner),
            None => EmbedOption::Clear,
        }
    }
}
