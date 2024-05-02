use bathbot_psql::model::osu::ArtistTitle;
use bathbot_util::string_cmp::{gestalt_pattern_matching, levenshtein_similarity};
use eyre::{Report, Result};

use crate::core::Context;

pub struct GameMapset {
    pub mapset_id: u32,
    artist: Box<str>,
    title: Box<str>,
    title_adjusted: Option<Box<str>>,
}

impl GameMapset {
    pub async fn new(mapset_id: u32) -> Result<Self> {
        let ArtistTitle { artist, title } = match Context::osu_map().artist_title(mapset_id).await {
            Ok(mut artist_title) => {
                artist_title.title.make_ascii_lowercase();
                artist_title.artist.make_ascii_lowercase();

                artist_title
            }
            Err(err) => return Err(Report::new(err).wrap_err("failed to get artist and title")),
        };

        let title_adjusted = if let (Some(open), Some(close)) = (title.find('('), title.rfind(')'))
        {
            let mut title_ = title.clone();
            title_.replace_range(open..=close, "");

            if let Some(idx) = title_.find("feat.").or_else(|| title_.find("ft.")) {
                title_.truncate(idx);
            }

            let trimmed = title_.trim();

            if trimmed.len() < title_.len() {
                Some(trimmed.to_owned())
            } else {
                Some(title_)
            }
        } else {
            title
                .find("feat.")
                .or_else(|| title.find("ft."))
                .map(|idx| title[..idx].trim_end().to_owned())
        };

        let mapset = Self {
            mapset_id,
            artist: artist.into_boxed_str(),
            title: title.into_boxed_str(),
            title_adjusted: title_adjusted.map(String::into_boxed_str),
        };

        Ok(mapset)
    }

    pub fn title(&self) -> &str {
        match self.title_adjusted.as_deref() {
            Some(title) => title,
            None => self.title.as_ref(),
        }
    }

    pub fn artist(&self) -> &str {
        self.artist.as_ref()
    }

    pub fn matches_title(&self, content: &str, difficulty: f32) -> Option<bool> {
        self.title_adjusted
            .as_deref()
            .and_then(|title| Self::matches(title, content, difficulty))
            .or_else(|| Self::matches(self.title.as_ref(), content, difficulty))
    }

    pub fn matches_artist(&self, content: &str, difficulty: f32) -> Option<bool> {
        Self::matches(self.artist.as_ref(), content, difficulty)
    }

    fn matches(src: &str, content: &str, difficulty: f32) -> Option<bool> {
        if src == content {
            Some(true)
        } else if levenshtein_similarity(src, content) > difficulty
            || gestalt_pattern_matching(src, content) > difficulty + 0.1
        {
            Some(false)
        } else {
            None
        }
    }
}
