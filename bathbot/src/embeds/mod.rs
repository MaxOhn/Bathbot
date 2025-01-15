use bathbot_util::EmbedBuilder;

pub use self::{osu::*, utility::*};

mod osu;
mod utility;

pub trait EmbedData {
    fn build(self) -> EmbedBuilder;
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
