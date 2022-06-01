use eyre::Report;
use rosu_v2::prelude::BeatmapsetCompact;

use crate::{
    core::Context,
    util::{gestalt_pattern_matching, levenshtein_similarity},
};

use super::GameResult;

pub struct GameMapset {
    pub mapset_id: u32,
    artist: String,
    title: String,
    title_adjusted: Option<String>,
}

impl GameMapset {
    pub async fn new(ctx: &Context, mapset_id: u32) -> GameResult<Self> {
        let (title, artist) = {
            let mapset_fut = ctx.psql().get_beatmapset::<BeatmapsetCompact>(mapset_id);

            if let Ok(mapset) = mapset_fut.await {
                (mapset.title.to_lowercase(), mapset.artist.to_lowercase())
            } else {
                let mapset = ctx.osu().beatmapset(mapset_id).await?;

                if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                    warn!("{:?}", Report::new(err));
                }

                (mapset.title.to_lowercase(), mapset.artist.to_lowercase())
            }
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
            artist,
            mapset_id,
            title,
            title_adjusted,
        };

        Ok(mapset)
    }

    pub fn title(&self) -> &str {
        match self.title_adjusted.as_deref() {
            Some(title) => title,
            None => &self.title,
        }
    }

    pub fn artist(&self) -> &str {
        &self.artist
    }

    pub fn matches_title(&self, content: &str, difficulty: f32) -> Option<bool> {
        self.title_adjusted
            .as_deref()
            .and_then(|title| Self::matches(title, content, difficulty))
            .or_else(|| Self::matches(&self.title, content, difficulty))
    }

    pub fn matches_artist(&self, content: &str, difficulty: f32) -> Option<bool> {
        Self::matches(&self.artist, content, difficulty)
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
