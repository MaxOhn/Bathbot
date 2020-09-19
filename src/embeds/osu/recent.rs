use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, with_comma_u64},
        osu::{grade_completion_mods, prepare_beatmap_file, unchoke_score},
        ScoreExt,
    },
    BotResult, Context,
};

use chrono::{DateTime, Utc};
use rosu::models::{Beatmap, GameMode, Grade, Score, User};
use std::fmt::Write;
use twilight_embed_builder::{
    author::EmbedAuthorBuilder, builder::EmbedBuilder, image_source::ImageSource,
};
use twilight_model::channel::embed::EmbedField;

#[derive(Clone)]
pub struct RecentEmbed {
    description: Option<String>,
    title: String,
    url: String,
    author: Author,
    footer: Footer,
    timestamp: DateTime<Utc>,
    thumbnail: ImageSource,
    image: ImageSource,

    stars: f32,
    grade_completion_mods: String,
    score: String,
    acc: f32,
    ago: String,
    pp: String,
    combo: String,
    hits: String,
    if_fc: Option<(String, String, String)>,
    map_info: String,
}

impl RecentEmbed {
    pub async fn new(
        ctx: &Context,
        user: &User,
        score: &Score,
        map: &Beatmap,
        personal: Option<&[Score]>,
        global: Option<&[Score]>,
    ) -> BotResult<Self> {
        let calculations = Calculations::all();
        let mut calculator = PPCalculator::new().score(score).map(map);
        let async_work = async {
            let personal_idx =
                personal.and_then(|personal| personal.iter().position(|s| s == score));
            let global_idx = global.and_then(|global| global.iter().position(|s| s == score));
            let description = if personal_idx.is_some() || global_idx.is_some() {
                let mut description = String::with_capacity(25);
                description.push_str("__**");
                if let Some(idx) = personal_idx {
                    let _ = write!(description, "Personal Best #{}", idx + 1);
                    if global_idx.is_some() {
                        description.reserve(19);
                        description.push_str(" and ");
                    }
                }
                if let Some(idx) = global_idx {
                    let _ = write!(description, "Global Top #{}", idx + 1);
                }
                description.push_str("**__");
                Some(description)
            } else {
                None
            };
            let title = if map.mode == GameMode::MNA {
                format!("{} {}", osu::get_keys(score.enabled_mods, &map), map)
            } else {
                map.to_string()
            };
            let grade_completion_mods = grade_completion_mods(score, map);
            (description, title, grade_completion_mods)
        };
        let async_if_fc = async {
            let got_s = match score.grade {
                Grade::S | Grade::SH | Grade::X | Grade::XH => true,
                _ => false,
            };
            if map.mode == GameMode::STD && (!got_s || score.max_combo < map.max_combo.unwrap() - 5)
            {
                let mut unchoked = score.clone();
                unchoke_score(&mut unchoked, &map);
                let mut calculator = PPCalculator::new().score(&unchoked).map(map);
                if let Err(why) = calculator.calculate(Calculations::PP, None).await {
                    warn!("Error while calculating pp of <recent score: {}", why);
                    None
                } else {
                    let combo = osu::get_combo(&unchoked, map);
                    let hits = unchoked.hits_string(map.mode);
                    Some((calculator.pp(), combo, hits))
                }
            } else {
                None
            }
        };
        // Prepare map file here so that it's not requested potentially two times
        prepare_beatmap_file(map.beatmap_id).await?;
        let (calc_result, (description, title, grade_completion_mods), if_fc) = tokio::join!(
            calculator.calculate(calculations, Some(ctx)),
            async_work,
            async_if_fc
        );
        if let Err(why) = calc_result {
            warn!("Error while calculating <recent pp: {}", why);
        }
        let max_pp = calculator.max_pp();
        let if_fc = if_fc.map(|(pp, x, y)| (osu::get_pp(pp, max_pp), x, y));
        let stars = round(calculator.stars().unwrap_or(0.0));
        let (pp, combo, hits) = (
            osu::get_pp(calculator.pp(), max_pp),
            if map.mode == GameMode::MNA {
                let mut ratio = score.count_geki as f32;
                if score.count300 > 0 {
                    ratio /= score.count300 as f32
                }
                format!("**{}x** / {:.2}", &score.max_combo, ratio)
            } else {
                osu::get_combo(score, map)
            },
            score.hits_string(map.mode),
        );
        let footer = Footer::new(format!(
            "{:?} map by {}, played",
            map.approval_status, map.creator
        ))
        .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));
        Ok(Self {
            description,
            title,
            url: format!("{}b/{}", OSU_BASE, map.beatmap_id),
            author: osu::get_user_author(&user),
            footer,
            timestamp: score.date,
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
                .unwrap(),
            image: ImageSource::url(format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ))
            .unwrap(),
            grade_completion_mods,
            stars,
            score: with_comma_u64(score.score as u64),
            acc: round(score.accuracy(map.mode)),
            ago: how_long_ago(&score.date),
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(&map),
            if_fc,
        })
    }
}

impl EmbedData for RecentEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        let mut fields = vec![
            ("Grade".to_owned(), self.grade_completion_mods.clone(), true),
            ("Score".to_owned(), self.score.clone(), true),
            ("Acc".to_owned(), format!("{}%", self.acc), true),
            ("PP".to_owned(), self.pp.clone(), true),
        ];
        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;
        fields.push((
            if mania { "Combo / Ratio" } else { "Combo" }.to_owned(),
            self.combo.clone(),
            true,
        ));
        fields.push(("Hits".to_owned(), self.hits.clone(), true));
        if let Some((pp, combo, hits)) = &self.if_fc {
            fields.push(("**If FC**: PP".to_owned(), pp.clone(), true));
            fields.push(("Combo".to_owned(), combo.clone(), true));
            fields.push(("Hits".to_owned(), hits.clone(), true));
        }
        fields.push(("Map Info".to_owned(), self.map_info.clone(), false));
        Some(fields)
    }
    fn minimize(&self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();
        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );
        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);
        let title = format!("{} [{}★]", self.title, self.stars);
        if self.description.is_some() {
            eb = eb.description(self.description.as_ref().unwrap()).unwrap();
        }
        let ab = EmbedAuthorBuilder::new()
            .name(&self.author.name)
            .unwrap()
            .url(self.author.url.as_ref().unwrap())
            .icon_url(self.author.icon_url.clone().unwrap());
        eb.color(DARK_GREEN)
            .unwrap()
            .thumbnail(self.thumbnail.clone())
            .title(title)
            .unwrap()
            .url(&self.url)
            .field(EmbedField {
                name,
                value,
                inline: false,
            })
            .author(ab)
    }
}
