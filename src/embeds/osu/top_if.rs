use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::with_comma_u64,
        ScoreExt,
    },
    Context,
};

use rosu::model::{Beatmap, GameMode, Score, User};
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
        ctx: &Context,
        user: &User,
        scores_data: S,
        mode: GameMode,
        adjusted_pp: f32,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score, Beatmap)>,
    {
        let pp_diff = (100.0 * (adjusted_pp - user.pp_raw)).round() / 100.0;
        let mut description = String::with_capacity(512);
        for (idx, score, map) in scores_data {
            let mut calculator = PPCalculator::new().score(score).map(map);
            if let Err(why) = calculator.calculate(Calculations::all(), Some(ctx)).await {
                warn!("Error while calculating pp for top: {}", why);
            }
            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));
            let pp = osu::get_pp(calculator.pp(), calculator.max_pp());
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
                mods = osu::get_mods(score.enabled_mods),
                stars = stars,
                grade = score.grade_emote(mode),
                pp = pp,
                acc = score.acc_string(mode),
                score = with_comma_u64(score.score as u64),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(mode),
                ago = how_long_ago(&score.date)
            );
        }
        description.pop();
        let title = format!(
            "Total pp: {} → **{}pp** ({:+})",
            user.pp_raw, adjusted_pp, pp_diff
        );
        Self {
            title,
            description,
            author: osu::get_user_author(user),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
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
