macro_rules! author {
    ($user:ident) => {{
        let stats = $user.statistics.as_ref().expect("no statistics on user");

        let text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = $user.username,
            pp = crate::util::numbers::with_comma_float(stats.pp),
            global = crate::util::numbers::with_comma_uint(stats.global_rank.unwrap_or(0)),
            country = $user.country_code,
            national = stats.country_rank.unwrap_or(0)
        );

        Author::new(text)
            .url(format!(
                "{}u/{}",
                crate::util::constants::OSU_BASE,
                $user.user_id
            ))
            .icon_url(crate::util::osu::flag_url($user.country_code.as_str()))
    }};
}

macro_rules! field {
    ($name:expr, $value:expr, $inline:expr) => {
        twilight_model::channel::embed::EmbedField {
            name: $name.into(),
            value: $value,
            inline: $inline,
        }
    };
}

#[allow(unused_macros)]
macro_rules! impl_builder {
    // Only through reference
    (&$ty:ty { $($field:ident,)+ }) => {
        impl crate::embeds::EmbedData for $ty {
            impl_builder!(SUB &$ty { $($field,)+ });
        }
    };

    // Only through ownership
    ($ty:ty { $($field:ident,)+ }) => {
        impl crate::embeds::EmbedData for $ty {
            impl_builder!(SUB $ty { $($field,)+ });
        }
    };

    // Through both reference and ownership
    (!$ty:ty { $($field:ident,)+ }) => {
        impl crate::embeds::EmbedData for $ty {
            impl_builder!(SUB &$ty { $($field,)+ });
            impl_builder!(SUB $ty { $($field,)+ });
        }
    };

    (SUB &$ty:ty { $($field:ident,)+ }) => {
        fn as_builder(&self) -> crate::embeds::EmbedBuilder {
            crate::embeds::EmbedBuilder::new()
                $(.$field(&self.$field))+
        }
    };

    (SUB $ty:ty { $($field:ident,)+ }) => {
        fn into_builder(self) -> crate::embeds::EmbedBuilder {
            crate::embeds::EmbedBuilder::new()
                $(.$field(self.$field))+
        }
    };

    // Without trailing comma
    (&$ty:ty { $($field:ident),+ }) => {
        impl_builder!(&$ty { $($field,)+ });
    };

    ($ty:ty { $($field:ident),+ }) => {
        impl_builder!($ty { $($field,)+ });
    };
}

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

use crate::util::{constants::DARK_GREEN, datetime};

use chrono::{DateTime, Utc};
use twilight_model::channel::embed::{
    Embed, EmbedAuthor, EmbedField, EmbedFooter, EmbedImage, EmbedThumbnail,
};

type EmbedFields = Vec<EmbedField>;

pub trait EmbedData: Send + Sync + Sized {
    fn as_builder(&self) -> EmbedBuilder {
        panic!("`as_builder` not implemented")
    }

    fn into_builder(self) -> EmbedBuilder {
        panic!("`into_builder` not implemented")
    }
}

#[inline]
fn validate_image_url(url: &str) {
    debug_assert!(
        url.starts_with("http:") || url.starts_with("https:"),
        "image url of embeds must start with `http:` or `https:`, got `{}`",
        url
    );
}

#[inline]
pub fn attachment(filename: impl AsRef<str>) -> String {
    #[cfg(debug_assert)]
    match filename.rfind('.') {
        Some(idx) => {
            if filename.get(idx + 1..).map(str::is_empty).is_none() {
                panic!("expected non-empty extension for attachment");
            }
        }
        None => panic!("expected extension for attachment"),
    }

    format!("attachment://{}", filename.as_ref())
}

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
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn build(mut self) -> Embed {
        self.0.kind.push_str("rich");

        self.0
    }

    #[inline]
    pub fn author(mut self, author: impl Into<EmbedAuthor>) -> Self {
        self.0.author.replace(author.into());

        self
    }

    #[inline]
    pub fn color(mut self, color: u32) -> Self {
        self.0.color.replace(color);

        self
    }

    #[inline]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.0.description.replace(description);

        self
    }

    #[inline]
    pub fn fields(mut self, fields: EmbedFields) -> Self {
        self.0.fields = fields;

        self
    }

    #[inline]
    pub fn footer(mut self, footer: impl Into<EmbedFooter>) -> Self {
        self.0.footer.replace(footer.into());

        self
    }

    #[inline]
    pub fn image(mut self, image: impl Into<String>) -> Self {
        let url = image.into();

        if !url.is_empty() {
            let image = EmbedImage {
                height: None,
                width: None,
                proxy_url: None,
                url: Some(url),
            };

            self.0.image.replace(image);
        }

        self
    }

    #[inline]
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        let timestamp = datetime::date_to_string(&timestamp);
        self.0.timestamp.replace(timestamp);

        self
    }

    #[inline]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title.replace(title.into());

        self
    }

    #[inline]
    pub fn thumbnail(mut self, thumbnail: impl Into<String>) -> Self {
        let url = thumbnail.into();

        if !url.is_empty() {
            let thumbnail = EmbedThumbnail {
                height: None,
                width: None,
                proxy_url: None,
                url: Some(url),
            };

            self.0.thumbnail.replace(thumbnail);
        }

        self
    }

    #[inline]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.0.url.replace(url.into());

        self
    }
}

#[derive(Clone)]
pub struct Footer(EmbedFooter);

impl Footer {
    #[inline]
    pub fn new(text: impl Into<String>) -> Self {
        Self(EmbedFooter {
            text: text.into(),
            icon_url: None,
            proxy_icon_url: None,
        })
    }

    #[inline]
    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        let icon_url = icon_url.into();
        validate_image_url(&icon_url);
        self.0.icon_url.replace(icon_url);

        self
    }

    #[inline]
    pub fn into_footer(self) -> EmbedFooter {
        self.0
    }

    #[inline]
    pub fn as_footer(&self) -> &EmbedFooter {
        &self.0
    }
}

impl From<Footer> for EmbedFooter {
    #[inline]
    fn from(footer: Footer) -> Self {
        footer.into_footer()
    }
}

impl From<&Footer> for EmbedFooter {
    #[inline]
    fn from(footer: &Footer) -> Self {
        footer.as_footer().to_owned()
    }
}

#[derive(Clone)]
pub struct Author(EmbedAuthor);

impl Author {
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self(EmbedAuthor {
            name: Some(name.into()),
            url: None,
            icon_url: None,
            proxy_icon_url: None,
        })
    }

    #[inline]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.0.url.replace(url.into());

        self
    }

    #[inline]
    pub fn icon_url(mut self, icon_url: impl Into<String>) -> Self {
        let icon_url = icon_url.into();
        validate_image_url(&icon_url);
        self.0.icon_url.replace(icon_url);

        self
    }

    #[inline]
    pub fn into_author(self) -> EmbedAuthor {
        self.0
    }

    #[inline]
    pub fn as_author(&self) -> &EmbedAuthor {
        &self.0
    }
}

impl From<Author> for EmbedAuthor {
    #[inline]
    fn from(author: Author) -> Self {
        author.into_author()
    }
}

impl From<&Author> for EmbedAuthor {
    #[inline]
    fn from(author: &Author) -> Self {
        author.as_author().to_owned()
    }
}
