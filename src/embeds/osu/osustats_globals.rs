use crate::{
    custom_client::OsuStatsScore,
    embeds::{osu, Author, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::OSU_BASE, datetime::how_long_ago_dynamic, numbers::with_comma_int,
        osu::grade_emote, ScoreExt,
    },
};

use eyre::Report;
use rosu_v2::model::user::User;
use std::{collections::BTreeMap, fmt::Write};

pub struct OsuStatsGlobalsEmbed {
    description: String,
    thumbnail: String,
    author: Author,
    footer: Footer,
}

impl OsuStatsGlobalsEmbed {
    pub async fn new(
        user: &User,
        scores: &BTreeMap<usize, OsuStatsScore>,
        total: usize,
        pages: (usize, usize),
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: author!(user),
                thumbnail: user.avatar_url.to_owned(),
                footer: Footer::new("Page 1/1 ~ Total scores: 0"),
                description: "No scores with these parameters were found".to_owned(),
            };
        }

        let index = (pages.0 - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        for (_, score) in entries {
            let grade = grade_emote(score.grade);
            let calculations = Calculations::all();
            let mut calculator = PPCalculator::new().score(score).map(&score.map);

            if let Err(why) = calculator.calculate(calculations).await {
                let report = Report::new(why).wrap_err("error while calcualting pp for osg");
                warn!("{:?}", report);
            }

            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));
            let pp = osu::get_pp(calculator.pp(), calculator.max_pp());
            let mut combo = format!("**{}x**/", score.max_combo);

            match score.map.max_combo {
                Some(amount) => {
                    let _ = write!(combo, "{}x", amount);
                }

                None => combo.push('-'),
            }

            let _ = writeln!(
                description,
                "**[#{rank}] [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}%) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                rank = score.position,
                title = score.map.title,
                version = score.map.version,
                base = OSU_BASE,
                id = score.map.beatmap_id,
                mods = osu::get_mods(score.enabled_mods),
                stars = stars,
                grade = grade,
                pp = pp,
                acc = score.accuracy,
                score = with_comma_int(score.score),
                combo = combo,
                hits = score.hits_string(score.map.mode),
                ago = how_long_ago_dynamic(&score.date)
            );
        }

        let footer = Footer::new(format!(
            "Page {}/{} ~ Total scores: {}",
            pages.0, pages.1, total
        ));

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

impl_builder!(OsuStatsGlobalsEmbed {
    author,
    description,
    footer,
    thumbnail,
});
