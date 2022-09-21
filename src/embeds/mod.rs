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

macro_rules! fields {
    // Push fields to a vec
    ($fields:ident {
        $($name:expr, $value:expr, $inline:expr);+
    }) => {
        fields![$fields { $($name, $value, $inline;)+ }]
    };

    ($fields:ident {
        $($name:expr, $value:expr, $inline:expr;)+
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
