macro_rules! fields {
    // Push fields to a vec
    ($fields:ident {
        $($name:expr, $value:expr, $inline:expr $(;)? )+
    }) => {
        $(
            $fields.push(
                twilight_model::channel::embed::EmbedField {
                    name: $name.into(),
                    value: $value,
                    inline: $inline,
                }
            );
        )+
    };

    // Create a new vec of fields
    ($($name:expr, $value:expr, $inline:expr);+) => {
        fields![$($name, $value, $inline;)+]
    };

    ($($name:expr, $value:expr, $inline:expr;)+) => {
        vec![
            $(
                twilight_model::channel::embed::EmbedField {
                    name: $name.into(),
                    value: $value,
                    inline: $inline,
                },
            )+
        ]
    };
}

use twilight_model::channel::embed::Embed;

pub use self::{fun::*, osu::*, utility::*};

#[cfg(feature = "osutracking")]
pub use self::tracking::*;

#[cfg(feature = "twitchtracking")]
pub use self::twitch::*;

mod fun;
mod osu;
mod twitch;
mod utility;

#[cfg(feature = "osutracking")]
mod tracking;

pub trait EmbedData {
    fn build(self) -> Embed;
}

impl EmbedData for Embed {
    fn build(self) -> Embed {
        self
    }
}

pub fn attachment(filename: impl AsRef<str>) -> String {
    let filename = filename.as_ref();

    #[cfg(debug_assertions)]
    if filename
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty())
        .is_none()
    {
        panic!("expected non-empty extension for attachment");
    }

    format!("attachment://{filename}")
}
