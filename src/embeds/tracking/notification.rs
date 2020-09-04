use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, with_comma_int},
        osu::grade_completion_mods,
        ScoreExt,
    },
    Context,
};

use chrono::{DateTime, Utc};
use rosu::models::{Beatmap, GameMode, Score, User};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
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
    pub async fn new(ctx: &Context, user: &User, score: &Score, map: &Beatmap, idx: usize) -> Self {
        let description = format!("__**Personal Best #{}**__", idx + 1);
        let calculations = Calculations::MAX_PP | Calculations::STARS;
        let mut calculator = PPCalculator::new().score(score).map(map);
        if let Err(why) = calculator.calculate(calculations, Some(ctx)).await {
            warn!("Error while calculating pp for tracking: {}", why);
        }
        let stars = round(calculator.stars().unwrap_or(0.0));
        let title = if map.mode == GameMode::MNA {
            format!(
                "{} {} [{}★]",
                osu::get_keys(score.enabled_mods, &map),
                map,
                stars
            )
        } else {
            format!("{} [{}★]", map, stars)
        };
        let name = format!(
            "{}\t{}\t({}%)",
            grade_completion_mods(score, map),
            with_comma_int(score.score),
            round(score.accuracy(map.mode))
        );
        let value = format!(
            "{} [ {} ] {}",
            osu::get_pp(score.pp, calculator.max_pp()),
            if map.mode == GameMode::MNA {
                let mut ratio = score.count_geki as f32;
                if score.count300 > 0 {
                    ratio /= score.count300 as f32
                }
                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                osu::get_combo(score, map)
            },
            score.hits_string(map.mode),
        );
        let footer = Footer::new(format!(
            "Mapped by {}, played {}",
            map.creator,
            how_long_ago(&score.date)
        ))
        .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));
        Self {
            title,
            footer,
            description,
            timestamp: score.date,
            fields: vec![(name, value, false)],
            url: format!("{}b/{}", OSU_BASE, map.beatmap_id),
            author: super::super::osu::get_user_author(user),
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
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
