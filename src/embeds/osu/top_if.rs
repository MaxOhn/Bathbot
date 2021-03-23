use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::with_comma_uint,
        ScoreExt,
    },
};

use rosu_v2::prelude::{GameMode, Score, User};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct TopIfEmbed {
    title: String,
    description: String,
    author: Author,
    thumbnail: ImageSource,
    footer: Footer,
}

impl TopIfEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores_data: S,
        mode: GameMode,
        pre_pp: f32,
        post_pp: f32,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score, Option<f32>)>,
    {
        let pp_diff = (100.0 * (post_pp - pre_pp)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for (idx, score, max_pp) in scores_data {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let stars = osu::get_stars(map.stars);

            let pp = match max_pp {
                Some(max_pp) => osu::get_pp(score.pp, Some(*max_pp)),
                None => osu::get_pp(None, None),
            };

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
                grade = score.grade_emote(mode),
                pp = pp,
                acc = score.acc_string(mode),
                score = with_comma_uint(score.score),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(mode),
                ago = how_long_ago(&score.created_at)
            );
        }

        description.pop();

        Self {
            description,
            author: author!(user),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
            title: format!("Total pp: {} → **{}pp** ({:+})", pre_pp, post_pp, pp_diff),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for TopIfEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
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
