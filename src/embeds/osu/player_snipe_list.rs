use crate::{
    custom_client::SnipeScore,
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    unwind_error,
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, with_comma_u64},
    },
};

use rosu::model::{Beatmap, User};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write,
};
use twilight_embed_builder::image_source::ImageSource;

pub struct PlayerSnipeListEmbed {
    description: String,
    thumbnail: ImageSource,
    author: Author,
    footer: Footer,
}

impl PlayerSnipeListEmbed {
    pub async fn new(
        user: &User,
        scores: &BTreeMap<usize, SnipeScore>,
        maps: &HashMap<u32, Beatmap>,
        total: usize,
        pages: (usize, usize),
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: osu::get_user_author(user),
                thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
                footer: Footer::new("Page 1/1 ~ Total #1 scores: 0"),
                description: String::from("No scores were found"),
            };
        }
        let index = (pages.0 - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);
        for (idx, score) in entries {
            let map = maps
                .get(&score.beatmap_id)
                .expect("missing beatmap for psl embed");
            let calculations = Calculations::MAX_PP;
            let mut calculator = PPCalculator::new().map(map).mods(score.mods);
            if let Err(why) = calculator.calculate(calculations, None).await {
                unwind_error!(warn, why, "Error while calculating pp for psl: {}");
            }
            let pp = osu::get_pp(score.pp, calculator.max_pp());
            let count_300 =
                map.count_objects() - score.count_100 - score.count_50 - score.count_miss;
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {pp} ~ ({acc}%) ~ {score}\n{{{n300}/{n100}/{n50}/{nmiss}}} ~ {ago}",
                idx = idx + 1,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = score.beatmap_id,
                mods = osu::get_mods(score.mods),
                stars = osu::get_stars(score.stars),
                pp = pp,
                acc = round(score.accuracy),
                score = with_comma_u64(score.score as u64),
                n300 = count_300,
                n100 = score.count_100,
                n50 = score.count_50,
                nmiss = score.count_miss,
                ago = how_long_ago(&score.score_date)
            );
        }
        Self {
            description,
            author: osu::get_user_author(&user),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
            footer: Footer::new(format!(
                "Page {}/{} ~ Total scores: {}",
                pages.0, pages.1, total
            )),
        }
    }
}

impl EmbedData for PlayerSnipeListEmbed {
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
