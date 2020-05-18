use super::util;
use crate::util::{
    datetime::{date_to_string, how_long_ago},
    discord::CacheData,
    globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    numbers::{round, round_and_comma, with_comma_u64},
    osu,
    pp::PPProvider,
    Error,
};

use rosu::models::{Beatmap, GameMode, Grade, Score, User};
use serenity::{builder::CreateEmbed, utils::Colour};
use std::{fmt::Write, sync::Arc};

pub struct RecentData {
    pub description: Option<String>,
    pub title: String,
    pub title_url: String,
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub stars: String,
    pub grade_completion_mods: String,
    pub score: String,
    pub acc: String,
    pub ago: String,
    pub pp: String,
    pub combo: String,
    pub hits: String,
    pub if_fc: Option<(String, String, String)>,
    pub map_info: String,
    pub footer_url: String,
    pub footer_text: String,
    pub timestamp: String,
    pub thumbnail: String,
    pub image: String,
}

impl RecentData {
    pub fn build<'d, 'e>(&'d self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        if self.description.is_some() {
            embed.description(&self.description.as_ref().unwrap());
        }
        embed
            .color(Colour::DARK_GREEN)
            .title(&self.title)
            .url(&self.title_url)
            .timestamp(self.timestamp.clone())
            .image(&self.image)
            .footer(|f| f.icon_url(&self.footer_url).text(&self.footer_text))
            .fields(vec![
                ("Grade", &self.grade_completion_mods, true),
                ("Score", &self.score, true),
                ("Acc", &self.acc, true),
                ("PP", &self.pp, true),
            ])
            .author(|a| {
                a.icon_url(&self.author_icon)
                    .url(&self.author_url)
                    .name(&self.author_text)
            });
        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;
        embed.field(
            if mania { "Combo / Ratio" } else { "Combo" },
            &self.combo,
            true,
        );
        embed.field("Hits", &self.hits, true);
        if let Some((pp, combo, hits)) = &self.if_fc {
            embed.field("**If FC**: PP", &pp, true);
            embed.field("Combo", &combo, true);
            embed.field("Hits", &hits, true);
        }
        embed.field("Map Info", &self.map_info, false)
    }

    pub fn minimize<'d, 'e>(&'d self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        let name = format!(
            "{}\t{}\t({})\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );
        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);
        let title = format!("{} [{}]", self.title, self.stars);
        if self.description.is_some() {
            embed.description(&self.description.as_ref().unwrap());
        }
        embed
            .color(Colour::DARK_GREEN)
            .field(name, value, false)
            .thumbnail(&self.thumbnail)
            .title(title)
            .url(&self.title_url)
            .author(|a| {
                a.icon_url(&self.author_icon)
                    .url(&self.author_url)
                    .name(&self.author_text)
            })
    }

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
            format!("{} {}", util::get_keys(&score.enabled_mods, &map), map)
        } else {
            map.to_string()
        };
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let author_icon = format!("{}{}", AVATAR_URL, user.user_id);
        let author_url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        let cache = cache_data.cache().clone();
        let grade_completion_mods = util::get_grade_completion_mods(&score, &map, cache).await;
        let data = Arc::clone(cache_data.data());
        let mut pp_provider = match PPProvider::new(&score, &map, Some(data)).await {
            Ok(provider) => provider,
            Err(why) => {
                return Err(Error::Custom(format!(
                    "Something went wrong while creating PPProvider: {}",
                    why
                )))
            }
        };
        let (pp, combo, hits) = (
            util::get_pp(&score, &pp_provider),
            if map.mode == GameMode::MNA {
                let mut ratio = score.count_geki as f32;
                if score.count300 > 0 {
                    ratio /= score.count300 as f32
                }
                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                util::get_combo(&score, &map)
            },
            util::get_hits(&score, map.mode),
        );
        let got_s = match score.grade {
            Grade::S | Grade::SH | Grade::X | Grade::XH => true,
            _ => false,
        };
        let if_fc = if map.mode == GameMode::STD
            && (!got_s || score.max_combo < map.max_combo.unwrap() - 5)
        {
            let mut unchoked = score.clone();
            osu::unchoke_score(&mut unchoked, &map);
            if let Err(why) = pp_provider.recalculate(&unchoked, GameMode::STD) {
                warn!("Error while unchoking score for <recent: {}", why);
                None
            } else {
                let pp = util::get_pp(&unchoked, &pp_provider);
                let combo = util::get_combo(&unchoked, &map);
                let hits = util::get_hits(&unchoked, map.mode);
                Some((pp, combo, hits))
            }
        } else {
            None
        };
        Ok(Self {
            description,
            title,
            title_url,
            author_icon,
            author_url,
            author_text,
            grade_completion_mods,
            stars: util::get_stars(pp_provider.stars()),
            score: with_comma_u64(score.score as u64),
            acc: util::get_acc(&score, map.mode),
            ago: how_long_ago(&score.date),
            pp,
            combo,
            hits,
            map_info: util::get_map_info(&map),
            footer_url: format!("{}{}", AVATAR_URL, map.creator_id),
            footer_text: format!("{:?} map by {}", map.approval_status, map.creator),
            timestamp: date_to_string(&score.date),
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
            image: format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ),
            if_fc,
        })
    }
}
