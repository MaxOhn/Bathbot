use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{
        datetime::how_long_ago,
        numbers::{round_and_comma, with_comma_u64},
        pp::PPProvider,
        Error,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::prelude::Context;

pub struct ScoreMultiData {
    pub title: String,
    pub title_url: String,
    pub thumbnail: String,
    pub footer_url: String,
    pub footer_text: String,
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub fields: Vec<(String, String, bool)>,
}

impl ScoreMultiData {
    pub fn new(user: User, map: Beatmap, scores: Vec<Score>, ctx: &Context) -> Result<Self, Error> {
        let title = map.to_string();
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
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let mut fields = Vec::new();
        for (i, score) in scores.into_iter().enumerate() {
            let pp_provider = match PPProvider::new(&score, &map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
            let mut name = format!(
                "**{idx}.** {grade}\t[{stars}]\t{score}\t({acc})",
                idx = (i + 1).to_string(),
                grade = util::get_grade_completion_mods(&score, &map, ctx.cache.clone()),
                stars = util::get_stars(&map, pp_provider.oppai()),
                score = with_comma_u64(score.score as u64),
                acc = util::get_acc(&score, map.mode),
            );
            if map.mode == GameMode::MNA {
                name.push('\t');
                name.push_str(&util::get_keys(&score.enabled_mods, &map));
            }
            let value = format!(
                "{pp}\t[ {combo} ]\t {hits}\t{ago}",
                pp = util::get_pp(&score, &pp_provider, map.mode),
                combo = util::get_combo(&score, &map),
                hits = util::get_hits(&score, map.mode),
                ago = how_long_ago(&score.date)
            );
            fields.push((name, value, false));
        }
        Ok(Self {
            title,
            title_url,
            author_icon,
            author_url,
            author_text,
            footer_url,
            footer_text,
            thumbnail,
            fields,
        })
    }
}
