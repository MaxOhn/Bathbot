use crate::{
    custom_client::SnipePlayer,
    embeds::{attachment, osu, Author, EmbedFields, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{with_comma_float, with_comma_int},
        osu::grade_completion_mods,
        ScoreExt,
    },
};

use eyre::Report;
use rosu_v2::prelude::{GameMode, Score, User};
use std::fmt::Write;

pub struct PlayerSnipeStatsEmbed {
    description: String,
    thumbnail: String,
    title: &'static str,
    url: String,
    author: Author,
    footer: Footer,
    image: String,
    fields: EmbedFields,
}

impl PlayerSnipeStatsEmbed {
    pub async fn new(user: User, player: SnipePlayer, first_score: Option<Score>) -> Self {
        let footer_text = format!(
            "{:+} #1{} since last update",
            player.difference,
            if player.difference == 1 { "" } else { "s" }
        );

        let (description, fields) = if player.count_first == 0 {
            ("No national #1s :(".to_owned(), Vec::new())
        } else {
            let mut fields = EmbedFields::with_capacity(7);
            let mut description = String::with_capacity(256);

            let _ = writeln!(
                description,
                "**Total #1s: {}** | ranked: {} | loved: {}",
                player.count_first, player.count_ranked, player.count_loved
            );

            fields.push(field!(
                "Average PP:",
                with_comma_float(player.avg_pp).to_string(),
                true
            ));

            fields.push(field!(
                "Average acc:",
                format!("{:.2}%", player.avg_acc),
                true
            ));

            fields.push(field!(
                "Average stars:",
                format!("{:.2}★", player.avg_stars),
                true
            ));

            if let Some(score) = first_score {
                let map = score.map.as_ref().unwrap();

                let calculations = Calculations::all();
                let mut calculator = PPCalculator::new().score(&score).map(map);

                let (pp, max_pp, stars) = match calculator.calculate(calculations).await {
                    Ok(_) => (
                        calculator.pp(),
                        calculator.max_pp(),
                        calculator.stars().unwrap_or(0.0),
                    ),
                    Err(why) => {
                        let report =
                            Report::new(why).wrap_err("error while calculating pp for pss");
                        warn!("{:?}", report);

                        (None, None, map.stars)
                    }
                };

                let stars = osu::get_stars(stars);
                let pp = osu::get_pp(pp, max_pp);

                let value = format!(
                    "**[{map}]({base}b/{id})**\t\
                    {grade}\t[{stars}]\t{score}\t({acc})\t[{combo}]\t\
                    [{pp}]\t {hits}\t{ago}",
                    map = player.oldest_first.unwrap().map,
                    base = OSU_BASE,
                    id = map.map_id,
                    grade = grade_completion_mods(&score, map),
                    stars = stars,
                    score = with_comma_int(score.score),
                    acc = score.acc_string(GameMode::STD),
                    pp = pp,
                    combo = osu::get_combo(&score, map),
                    hits = score.hits_string(GameMode::STD),
                    ago = how_long_ago_dynamic(&score.created_at)
                );

                fields.push(field!("Oldest national #1:", value, false));
            }

            let mut count_mods = player.count_mods.unwrap();
            let mut value = String::with_capacity(count_mods.len() * 7);
            count_mods.sort_unstable_by(|(_, c1), (_, c2)| c2.cmp(c1));
            let mut iter = count_mods.into_iter();

            if let Some((first_mods, first_amount)) = iter.next() {
                let _ = write!(value, "`{first_mods}: {first_amount}`");
                let mut idx = 0;

                for (mods, amount) in iter {
                    match idx {
                        2 => {
                            idx = 0;
                            let _ = write!(value, " >\n`{mods}: {amount}`");
                        }
                        _ => {
                            idx += 1;
                            let _ = write!(value, " > `{mods}: {amount}`");
                        }
                    }
                }
            }

            fields.push(field!("Most used mods:", value, false));

            (description, fields)
        };

        let url = format!(
            "https://snipe.huismetbenen.nl/player/be/osu/{}",
            user.user_id
        );

        Self {
            url,
            fields,
            description,
            footer: Footer::new(footer_text),
            author: author!(user),
            title: "National #1 statistics",
            image: attachment("stats_graph.png"),
            thumbnail: user.avatar_url,
        }
    }
}

impl_builder!(PlayerSnipeStatsEmbed {
    author,
    description,
    fields,
    footer,
    image,
    thumbnail,
    title,
    url,
});
