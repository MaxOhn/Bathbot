use std::{collections::BTreeMap, fmt::Write};

use command_macros::EmbedData;
use eyre::Report;
use rosu_v2::model::user::User;

use crate::{
    core::Context,
    custom_client::OsuStatsScore,
    embeds::osu,
    pagination::Pages,
    pp::PpCalculator,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::with_comma_int,
        osu::grade_emote,
        CowUtils, ScoreExt,
    },
};

#[derive(EmbedData)]
pub struct OsuStatsGlobalsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl OsuStatsGlobalsEmbed {
    pub async fn new(
        user: &User,
        scores: &BTreeMap<usize, OsuStatsScore>,
        total: usize,
        ctx: &Context,
        pages: &Pages,
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: author!(user),
                thumbnail: user.avatar_url.to_owned(),
                footer: FooterBuilder::new("Page 1/1 ~ Total scores: 0"),
                description: "No scores with these parameters were found".to_owned(),
            };
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let index = (page - 1) * 5;

        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        for (_, score) in entries {
            let grade = grade_emote(score.grade);

            let (pp, max_pp, stars) = match PpCalculator::new(ctx, score.map.beatmap_id).await {
                Ok(base_calc) => {
                    let mut calc = base_calc.score(score);

                    let stars = calc.stars();
                    let max_pp = calc.max_pp();
                    let pp = calc.pp();

                    (Some(pp as f32), Some(max_pp as f32), stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (None, None, 0.0)
                }
            };

            let pp = osu::get_pp(pp, max_pp);
            let mut combo = format!("**{}x**/", score.max_combo);

            match score.map.max_combo {
                Some(amount) => {
                    let _ = write!(combo, "{amount}x");
                }

                None => combo.push('-'),
            }

            let _ = writeln!(
                description,
                "**[#{rank}] [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {pp} ~ ({acc}%) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                rank = score.position,
                title = score.map.title.cow_escape_markdown(),
                version = score.map.version.cow_escape_markdown(),
                id = score.map.beatmap_id,
                mods = osu::get_mods(score.enabled_mods),
                acc = score.accuracy,
                score = with_comma_int(score.score),
                hits = score.hits_string(score.map.mode),
                ago = how_long_ago_dynamic(&score.date)
            );
        }

        let footer = FooterBuilder::new(format!("Page {page}/{pages} ~ Total scores: {total}"));

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}
