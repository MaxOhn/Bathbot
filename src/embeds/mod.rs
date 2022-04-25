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

mod fun;
mod osu;
mod tracking;
mod twitch;
mod utility;

use twilight_model::channel::embed::{Embed, EmbedField};

pub use self::{fun::*, osu::*, tracking::*, twitch::*, utility::*};

type EmbedFields = Vec<EmbedField>;

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
