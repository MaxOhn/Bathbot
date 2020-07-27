use crate::{
    custom_client::OsuStatsScore,
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::with_comma_int,
        osu::grade_emote,
        ScoreExt,
    },
    BotResult, Context,
};

use rosu::models::User;
use std::{collections::BTreeMap, fmt::Write, sync::Arc};

#[derive(Clone)]
pub struct OsuStatsGlobalsEmbed {
    description: String,
    thumbnail: String,
    author: Author,
    footer: Footer,
}

impl OsuStatsGlobalsEmbed {
    pub async fn new(
        ctx: Arc<Context>,
        user: &User,
        scores: &BTreeMap<usize, OsuStatsScore>,
        total: usize,
        pages: (usize, usize),
    ) -> BotResult<Self> {
        if scores.is_empty() {
            return Ok(Self {
                author: osu::get_user_author(&user),
                thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
                footer: Footer::new(String::from("Page 1/1 ~ Total scores: 0")),
                description: String::from("No scores with these parameters were found"),
            });
        }
        let index = (pages.0 - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);
        for (_, score) in entries {
            let grade = grade_emote(score.grade, &ctx);
            let calculations = Calculations::PP | Calculations::MAX_PP | Calculations::STARS;
            let mut calculator = PPCalculator::new()
                .score(score)
                .map(&score.map)
                .ctx(ctx.clone());
            calculator.calculate(calculations).await?;
            let stars = osu::get_stars(calculator.stars().unwrap());
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
                ago = how_long_ago(&score.date)
            );
        }
        Ok(Self {
            description,
            author: osu::get_user_author(&user),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            footer: Footer::new(format!(
                "Page {}/{} ~ Total scores: {}",
                pages.0, pages.1, total
            )),
        })
    }
}

impl EmbedData for OsuStatsGlobalsEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
