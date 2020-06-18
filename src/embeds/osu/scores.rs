use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        datetime::how_long_ago,
        discord::CacheData,
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round_and_comma, with_comma_u64},
        pp::PPProvider,
    },
};

use failure::Error;
use rosu::models::{Beatmap, GameMode, Score, User};
use std::fmt::Write;

#[derive(Clone)]
pub struct ScoresEmbed {
    description: Option<String>,
    fields: Vec<(String, String, bool)>,
    thumbnail: String,
    footer: Footer,
    author: Author,
    title: String,
    url: String,
}

impl ScoresEmbed {
    pub async fn new<D>(
        user: User,
        map: Beatmap,
        scores: Vec<Score>,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        D: CacheData,
    {
        let description = if scores.is_empty() {
            Some("No scores found".to_string())
        } else {
            None
        };
        let mut fields = Vec::new();
        for (i, score) in scores.into_iter().enumerate() {
            let (stars, pp) = {
                let pp_provider = match PPProvider::new(&score, &map, Some(cache_data.data())).await
                {
                    Ok(provider) => provider,
                    Err(why) => bail!("Something went wrong while creating PPProvider: {}", why),
                };
                (
                    osu::get_stars(pp_provider.stars()),
                    osu::get_pp(&score, &pp_provider),
                )
            };
            let mut name = format!(
                "**{idx}.** {grade}\t[{stars}]\t{score}\t({acc})",
                idx = i + 1,
                grade = osu::get_grade_completion_mods(&score, &map, cache_data.cache()).await,
                stars = stars,
                score = with_comma_u64(score.score as u64),
                acc = osu::get_acc(&score, map.mode),
            );
            if map.mode == GameMode::MNA {
                let _ = write!(name, "\t{}", osu::get_keys(score.enabled_mods, &map));
            }
            let value = format!(
                "{pp}\t[ {combo} ]\t {hits}\t{ago}",
                pp = pp,
                combo = osu::get_combo(&score, &map),
                hits = osu::get_hits(&score, map.mode),
                ago = how_long_ago(&score.date)
            );
            fields.push((name, value, false));
        }
        let footer = Footer::new(format!("{:?} map by {}", map.approval_status, map.creator))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        let author = Author::new(author_text)
            .url(format!("{}u/{}", HOMEPAGE, user.user_id))
            .icon_url(format!("{}{}", AVATAR_URL, user.user_id));
        Ok(Self {
            description,
            footer,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
            title: map.to_string(),
            url: format!("{}b/{}", HOMEPAGE, map.beatmap_id),
            fields,
            author,
        })
    }
}

impl EmbedData for ScoresEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
}
