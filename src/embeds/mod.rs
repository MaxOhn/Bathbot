macro_rules! author {
    ($user:ident) => {{
        let stats = $user.statistics.as_ref().expect("no statistics on user");

        let text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = $user.username,
            pp = crate::util::numbers::with_comma_float(stats.pp),
            global = crate::util::numbers::with_comma_int(stats.global_rank.unwrap_or(0)),
            country = $user.country_code,
            national = stats.country_rank.unwrap_or(0)
        );

        let url = format!(
            "{}users/{}/{}",
            crate::util::constants::OSU_BASE,
            $user.user_id,
            $user.mode,
        );

        let icon = crate::util::osu::flag_url($user.country_code.as_str());

        AuthorBuilder::new(text).url(url).icon_url(icon)
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
        fn as_builder(&self) -> crate::util::builder::EmbedBuilder {
            crate::util::builder::EmbedBuilder::new()
                $(.$field(self.$field.clone()))+
        }
    };

    (SUB $ty:ty { $($field:ident,)+ }) => {
        fn into_builder(self) -> crate::util::builder::EmbedBuilder {
            crate::util::builder::EmbedBuilder::new()
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
mod tracking;
mod twitch;
mod utility;

use twilight_model::channel::embed::EmbedField;

use crate::util::builder::EmbedBuilder;

pub use self::{fun::*, osu::*, tracking::*, twitch::*, utility::*};

type EmbedFields = Vec<EmbedField>;

pub trait EmbedData: Send + Sync + Sized {
    fn as_builder(&self) -> EmbedBuilder {
        panic!("`as_builder` not implemented")
    }

    fn into_builder(self) -> EmbedBuilder {
        panic!("`into_builder` not implemented")
    }
}

impl EmbedData for EmbedBuilder {
    fn into_builder(self) -> EmbedBuilder {
        self
    }
}

fn validate_image_url(url: &str) {
    debug_assert!(
        url.starts_with("http:") || url.starts_with("https:"),
        "image url of embeds must start with `http:` or `https:`, got `{}`",
        url
    );
}

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
