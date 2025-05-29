use bathbot_util::EmbedBuilder;

pub use self::{osu::*, utility::*};

mod osu;
mod utility;

pub trait EmbedData {
    fn build(self) -> EmbedBuilder;
}
