use crate::{
    commands::osu::{CompareResult, MinMaxAvgBasic},
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::AVATAR_URL,
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        numbers::with_comma_int,
        osu::grade_emote,
    },
};

use chrono::Utc;
use rosu::models::{GameMode, Grade, User};
use std::{collections::BTreeMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct ProfileCompareEmbed {
    description: String,
}

impl ProfileCompareEmbed {
    pub fn new(user1: User, user2: User, result1: CompareResult, result2: CompareResult) -> Self {
        let description = String::from("TODO");
        Self { description }
    }
}

impl EmbedData for ProfileCompareEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
