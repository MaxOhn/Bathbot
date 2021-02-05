use crate::{
    custom_client::SnipePlayer,
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    unwind_error,
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{with_comma, with_comma_u64},
        osu::grade_completion_mods,
        ScoreExt,
    },
};

use rosu::model::{Beatmap, GameMode, Score, User};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct PlayerSnipeStatsEmbed {
    description: Option<String>,
    thumbnail: Option<ImageSource>,
    title: &'static str,
    author: Option<Author>,
    footer: Option<Footer>,
    image: Option<ImageSource>,
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
                "**Total #1s: {}** | ranked: {} | loved: {}",
                player.count_first, player.count_ranked, player.count_loved
            );
            fields.push((String::from("Average PP:"), with_comma(player.avg_pp), true));
            fields.push((
                String::from("Average acc:"),
                format!("{:.2}%", player.avg_acc),
                true,
            ));
            fields.push((
                String::from("Average stars:"),
                format!("{:.2}★", player.avg_stars),
                true,
            ));
            let (score, map) = first_score.unwrap(); // TODO: Fix this
            let calculations = Calculations::all();
            let mut calculator = PPCalculator::new().score(&score).map(&map);
            let (pp, max_pp, stars) = match calculator.calculate(calculations).await {
                Ok(_) => (
                    calculator.pp(),
                    calculator.max_pp(),
                    calculator.stars().unwrap_or(0.0),
                ),
                Err(why) => {
                    unwind_error!(warn, why, "Error while calculating pp: {}");
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
                score = with_comma_u64(score.score as u64),
                acc = score.acc_string(GameMode::STD),
                pp = pp,
                combo = osu::get_combo(&score, &map),
                hits = score.hits_string(GameMode::STD),
                ago = how_long_ago(&score.date)
            );
            fields.push((String::from("Oldest national #1:"), value, false));
            let mut count_mods = player.count_mods.unwrap();
            let mut value = String::with_capacity(count_mods.len() * 7);
            count_mods.sort_unstable_by(|(_, c1), (_, c2)| c2.cmp(c1));
            let mut iter = count_mods.into_iter();
            if let Some((first_mods, first_amount)) = iter.next() {
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
            }
            fields.push((String::from("Most used mods:"), value, false));
            (description, Some(fields))
        };

        Self {
            fields,
            description: Some(description),
            footer: Some(Footer::new(footer_text)),
            author: Some(osu::get_user_author(&user)),
            title: "National #1 statistics",
            image: Some(ImageSource::attachment("stats_graph.png").unwrap()),
            thumbnail: Some(ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap()),
        }
    }
}

impl EmbedData for PlayerSnipeStatsEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }
    fn title_owned(&mut self) -> Option<String> {
        Some(self.title.to_owned())
    }
    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }
    fn image_owned(&mut self) -> Option<ImageSource> {
        self.image.take()
    }
    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }
    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }
    fn fields_owned(self) -> Option<Vec<(String, String, bool)>> {
        self.fields
    }
}
