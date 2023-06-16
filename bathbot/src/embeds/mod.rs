use bathbot_util::EmbedBuilder;

#[cfg(feature = "osutracking")]
pub use self::tracking::*;
pub use self::{osu::*, utility::*};

mod osu;
mod utility;

#[cfg(feature = "osutracking")]
mod tracking;

pub trait EmbedData {
    fn build(self) -> EmbedBuilder;
}

// TODO
// impl EmbedData for Embed {
//     fn build(self) -> EmbedBuilder {
//         self
//     }
// }

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
