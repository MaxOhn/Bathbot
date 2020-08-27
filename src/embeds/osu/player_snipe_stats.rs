use crate::{
    custom_client::SnipePlayer,
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, round_and_comma, with_comma_int},
        osu::grade_completion_mods,
        ScoreExt,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct PlayerSnipeStatsEmbed {
    description: String,
    thumbnail: ImageSource,
    title: &'static str,
    author: Author,
    footer: Footer,
    image: ImageSource,
    fields: Option<Vec<(String, String, bool)>>,
}

impl PlayerSnipeStatsEmbed {
    pub async fn new(
        user: User,
        player: SnipePlayer,
        first_score: Option<(Score, Beatmap)>,
    ) -> Self {
        let footer_text = format!(
            "{:+} #1{} since last update",
            player.difference,
            if player.difference == 1 { "" } else { "s" }
        );
        let (description, fields) = if player.count_first == 0 {
            (String::from("No national #1s :("), None)
        } else {
            let mut fields = Vec::with_capacity(7);
            let mut description = String::with_capacity(256);
            let _ = writeln!(
                description,
                "**Total#1s: {}** | ranked: {} | loved: {}",
                player.count_first, player.count_ranked, player.count_loved
            );
            fields.push((
                String::from("Average PP:"),
                round_and_comma(player.avg_pp),
                true,
            ));
            fields.push((
                String::from("Average acc:"),
                round(player.avg_acc).to_string() + "%",
                true,
            ));
            fields.push((
                String::from("Average stars:"),
                round(player.avg_stars).to_string() + "★",
                true,
            ));
            let (score, map) = first_score.unwrap();
            let calculations = Calculations::all();
            let mut calculator = PPCalculator::new().score(&score).map(&map);
            let (pp, max_pp, stars) = match calculator.calculate(calculations, None).await {
                Ok(_) => (
                    calculator.pp(),
                    calculator.max_pp(),
                    calculator.stars().unwrap(),
                ),
                Err(why) => {
                    warn!("Error while calculating pp: {}", why);
                    (None, None, map.stars)
                }
            };
            let stars = osu::get_stars(stars);
            let pp = osu::get_pp(pp, max_pp);
            let value = format!(
                "**[{map}]({base}b/{id})**\t\
                {grade}\t[{stars}]\t{score}\t({acc})\t[{combo}]\t\
                [{pp}]\t {hits}\t{ago}",
                map = map,
                base = OSU_BASE,
                id = map.beatmap_id,
                grade = grade_completion_mods(&score, &map),
                stars = stars,
                score = with_comma_int(score.score),
                acc = score.acc_string(GameMode::STD),
                pp = pp,
                combo = osu::get_combo(&score, &map),
                hits = score.hits_string(GameMode::STD),
                ago = how_long_ago(&score.date)
            );
            fields.push((String::from("Oldest national #1:"), value, false));
            let mut count_mods = player.count_mods.unwrap();
            let mut value = String::with_capacity(count_mods.len() * 7);
            count_mods.sort_by(|(_, c1), (_, c2)| c2.cmp(c1));
            let mut iter = count_mods.into_iter();
            let (first_mods, first_amount) = iter.next().unwrap();
            let _ = write!(value, "`{}: {}`", first_mods, first_amount);
            let mut idx = 0;
            for (mods, amount) in iter {
                match idx {
                    2 => {
                        idx = 0;
                        let _ = write!(value, " >\n`{}: {}`", mods, amount);
                    }
                    _ => {
                        idx += 1;
                        let _ = write!(value, " > `{}: {}`", mods, amount);
                    }
                }
            }
            fields.push((String::from("Most used mods:"), value, false));
            (description, Some(fields))
        };
        Self {
            fields,
            description,
            footer: Footer::new(footer_text),
            author: osu::get_user_author(&user),
            title: "National #1 statistics",
            image: ImageSource::attachment("stats_graph.png").unwrap(),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for PlayerSnipeStatsEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        self.fields.clone()
    }
}
