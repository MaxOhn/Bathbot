use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::with_comma_u64,
        ScoreExt,
    },
};

use rosu_v2::prelude::{Score, User};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct TopEmbed {
    description: String,
    author: Author,
    thumbnail: ImageSource,
    footer: Footer,
}

impl TopEmbed {
    pub async fn new<'i, S>(user: &User, scores: S, pages: (usize, usize)) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let mut calculator = PPCalculator::new().score(score).map(map);
            let mut calculations = Calculations::MAX_PP | Calculations::STARS;

            if score.pp.is_none() {
                calculations |= Calculations::PP;
            }

            if let Err(why) = calculator.calculate(calculations).await {
                unwind_error!(warn, why, "Error while calculating pp for top: {}");
            }

            let pp = score.pp.or_else(|| calculator.pp());

            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));
            let pp = osu::get_pp(pp, calculator.max_pp());

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx,
                title = mapset.title,
                version = map.version,
                base = OSU_BASE,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                stars = stars,
                grade = score.grade_emote(score.mode),
                pp = pp,
                acc = score.acc_string(score.mode),
                score = with_comma_u64(score.score as u64),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(score.mode),
                ago = how_long_ago(&score.created_at)
            );
        }

        description.pop();

        Self {
            description,
            author: author!(user),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for TopEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }

    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
