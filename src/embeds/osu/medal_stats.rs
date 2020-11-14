use crate::{
    custom_client::{OsuMedals, OsuProfile},
    embeds::EmbedData,
    util::{
        constants::{FIELD_VALUE_SIZE, OSU_BASE},
        numbers::round,
    },
};

use cow_utils::CowUtils;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct MedalStatsEmbed {
    url: String,
    thumbnail: ImageSource,
    title: String,
    fields: Vec<(String, String, bool)>,
}

impl MedalStatsEmbed {
    pub fn new(profile: OsuProfile, medals: OsuMedals) -> Self {
        todo!()
    }
}

impl EmbedData for MedalStatsEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
