use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, with_comma_u64},
        osu::{grade_completion_mods, mode_emote},
        ScoreExt,
    },
};

use chrono::{DateTime, Utc};
use rosu_v2::prelude::{GameMode, Score, User};
use twilight_embed_builder::image_source::ImageSource;

pub struct TrackNotificationEmbed {
    fields: Vec<(String, String, bool)>,
    description: String,
    author: Author,
    title: String,
    url: String,
    thumbnail: ImageSource,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl TrackNotificationEmbed {
    pub async fn new(user: &User, score: &Score, idx: usize) -> Self {
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let description = format!("{} __**Personal Best #{}**__", mode_emote(map.mode), idx);
        let calculations = Calculations::MAX_PP | Calculations::STARS;
        let mut calculator = PPCalculator::new().score(score).map(map);

        if let Err(why) = calculator.calculate(calculations).await {
            warn!("Error while calculating pp for tracking: {}", why);
        }

        let stars = round(calculator.stars().unwrap_or(0.0));

        let title = if map.mode == GameMode::MNA {
            format!(
                "{} {} - {} [{}] [{}★]",
                osu::get_keys(score.mods, &map),
                mapset.artist,
                mapset.title,
                map.version,
                stars
            )
        } else {
            format!(
                "{} - {} [{}] [{}★]",
                mapset.artist, mapset.title, map.version, stars
            )
        };

        let name = format!(
            "{}\t{}\t({}%)",
            grade_completion_mods(score, map),
            with_comma_u64(score.score as u64),
            round(score.accuracy)
        );

        let value = format!(
            "{} [ {} ] {}",
            osu::get_pp(score.pp, calculator.max_pp()),
            if map.mode == GameMode::MNA {
                let mut ratio = score.statistics.count_geki as f32;

                if score.statistics.count_300 > 0 {
                    ratio /= score.statistics.count_300 as f32
                }

                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                osu::get_combo(score, map)
            },
            score.hits_string(map.mode),
        );

        let footer = Footer::new(format!(
            "Mapped by {}, played {}",
            mapset.creator_name,
            how_long_ago(&score.created_at)
        ))
        .icon_url(format!("{}{}", AVATAR_URL, mapset.creator_id));

        let author = author!(user).icon_url(format!("{}{}", AVATAR_URL, user.user_id));

        Self {
            title,
            footer,
            author,
            description,
            timestamp: score.created_at,
            fields: vec![(name, value, false)],
            url: format!("{}b/{}", OSU_BASE, map.map_id),
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id))
                .unwrap(),
        }
    }
}

impl EmbedData for TrackNotificationEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}
