use chrono::{DateTime, Utc};
use eyre::Report;
use rosu_v2::prelude::{GameMode, Score, User};

use crate::{
    embeds::{osu, Author, EmbedFields, Footer},
    pp::PpCalculator,
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago_text,
        numbers::{round, with_comma_int},
        osu::{grade_completion_mods, mode_emote},
        ScoreExt,
    },
};

pub struct TrackNotificationEmbed {
    fields: EmbedFields,
    description: String,
    author: Author,
    title: String,
    url: String,
    thumbnail: String,
    footer: Footer,
    timestamp: DateTime<Utc>,
}

impl TrackNotificationEmbed {
    pub async fn new(user: &User, score: &Score, idx: usize) -> Self {
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let description = format!("{} __**Personal Best #{idx}**__", mode_emote(map.mode));

        let (max_pp, stars) = match PpCalculator::new(map.map_id).await {
            Ok(mut calc) => {
                calc.score(score);

                let stars = calc.stars();
                let max_pp = calc.max_pp();

                (Some(max_pp as f32), round(stars as f32))
            }
            Err(err) => {
                warn!("{:?}", Report::new(err));

                (None, 0.0)
            }
        };

        let title = if map.mode == GameMode::MNA {
            format!(
                "{} {} - {} [{}] [{stars}★]",
                osu::get_keys(score.mods, map),
                mapset.artist,
                mapset.title,
                map.version,
            )
        } else {
            format!(
                "{} - {} [{}] [{stars}★]",
                mapset.artist, mapset.title, map.version
            )
        };

        let name = format!(
            "{}\t{}\t({}%)",
            grade_completion_mods(score, map),
            with_comma_int(score.score),
            round(score.accuracy)
        );

        let value = format!(
            "{} [ {} ] {}",
            osu::get_pp(score.pp, max_pp),
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
            how_long_ago_text(&score.created_at)
        ))
        .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

        let author = author!(user).icon_url(user.avatar_url.to_owned());

        Self {
            author,
            description,
            fields: vec![field!(name, value, false)],
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id),
            timestamp: score.created_at,
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id),
        }
    }
}

impl_builder!(TrackNotificationEmbed {
    author,
    description,
    fields,
    footer,
    thumbnail,
    timestamp,
    title,
    url,
});
