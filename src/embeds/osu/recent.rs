use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        datetime::how_long_ago,
        discord::CacheData,
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round, with_comma_u64},
        osu::unchoke_score,
        pp::PPProvider,
    },
    Error,
};

use chrono::{DateTime, Utc};
use rosu::models::{Beatmap, GameMode, Grade, Score, User};
use serenity::{builder::CreateEmbed, utils::Colour};
use std::fmt::Write;

#[derive(Clone)]
pub struct RecentEmbed {
    description: Option<String>,
    title: String,
    url: String,
    author: Author,
    footer: Footer,
    timestamp: DateTime<Utc>,
    thumbnail: String,
    image: String,

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
    pub async fn new<D>(
        user: &User,
        score: &Score,
        map: &Beatmap,
        personal: &[Score],
        global: &[Score],
        cache_data: D,
    ) -> Result<Self, Error>
    where
        D: CacheData,
    {
        let personal_idx = personal.iter().position(|s| s == score);
        let global_idx = global.iter().position(|s| s == score);
        let description = if personal_idx.is_some() || global_idx.is_some() {
            let mut description = String::from("__**");
            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{}", idx + 1);
                if global_idx.is_some() {
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
        let grade_completion_mods =
            osu::get_grade_completion_mods(&score, &map, cache_data.cache()).await;
        let mut pp_provider = match PPProvider::new(&score, &map, Some(cache_data.data())).await {
            Ok(provider) => provider,
            Err(why) => {
                return Err(Error::Custom(format!(
                    "Something went wrong while creating PPProvider: {}",
                    why
                )))
            }
        };
        let (pp, combo, hits) = (
            osu::get_pp(&score, &pp_provider),
            if map.mode == GameMode::MNA {
                let mut ratio = score.count_geki as f32;
                if score.count300 > 0 {
                    ratio /= score.count300 as f32
                }
                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                osu::get_combo(&score, &map)
            },
            osu::get_hits(&score, map.mode),
        );
        let got_s = match score.grade {
            Grade::S | Grade::SH | Grade::X | Grade::XH => true,
            _ => false,
        };
        let if_fc = if map.mode == GameMode::STD
            && (!got_s || score.max_combo < map.max_combo.unwrap() - 5)
        {
            let mut unchoked = score.clone();
            unchoke_score(&mut unchoked, &map);
            if let Err(why) = pp_provider.recalculate(&unchoked, GameMode::STD) {
                warn!("Error while unchoking score for <recent: {}", why);
                None
            } else {
                let pp = osu::get_pp(&unchoked, &pp_provider);
                let combo = osu::get_combo(&unchoked, &map);
                let hits = osu::get_hits(&unchoked, map.mode);
                Some((pp, combo, hits))
            }
        } else {
            None
        };
        let footer = Footer::new(format!("{:?} map by {}", map.approval_status, map.creator))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));
        Ok(Self {
            description,
            title,
            url: format!("{}b/{}", HOMEPAGE, map.beatmap_id),
            author: osu::get_user_author(&user),
            footer,
            timestamp: score.date,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
            image: format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ),

            grade_completion_mods,
            stars: round(pp_provider.stars()),
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
    fn image(&self) -> Option<&str> {
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
    fn minimize<'e>(&self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        let name = format!(
            "{}\t{}\t({})\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );
        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);
        let title = format!("{} [{}★]", self.title, self.stars);
        if self.description.is_some() {
            e.description(&self.description.as_ref().unwrap());
        }
        e.color(Colour::DARK_GREEN)
            .field(name, value, false)
            .thumbnail(&self.thumbnail)
            .title(title)
            .url(&self.url)
            .author(|a| {
                a.icon_url(self.author.icon_url.as_ref().unwrap())
                    .url(self.author.url.as_ref().unwrap())
                    .name(&self.author.name)
            })
    }
}
