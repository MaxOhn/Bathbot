use crate::{
    embeds::{osu, Author, EmbedData},
    scraper::OsuStatsScore,
    util::{
        datetime::how_long_ago,
        discord::CacheData,
        globals::{AVATAR_URL, HOMEPAGE},
        numbers::round_and_comma,
        numbers::with_comma_u64,
        osu::grade_emote,
        pp::{Calculations, PPCalculator},
    },
};

use failure::Error;
use rosu::models::User;
use std::{collections::BTreeMap, fmt::Write};

#[derive(Clone)]
pub struct OsuStatsGlobalsEmbed {
    description: String,
    thumbnail: String,
    author: Author,
}

impl OsuStatsGlobalsEmbed {
    pub async fn new<D>(
        user: &User,
        scores: &BTreeMap<usize, OsuStatsScore>,
        index: usize,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        D: CacheData,
    {
        let entries = scores.range(index..index + 10);
        let mut description = String::with_capacity(1024);
        for (idx, score) in entries {
            // let grade = { grade_emote(score.grade, cache_data.cache()).await };
            // let (stars, pp) = {
            //     let pp_provider = match PPProvider::new(&score, &map, Some(cache_data.data())).await
            //     {
            //         Ok(provider) => provider,
            //         Err(why) => bail!("Something went wrong while creating PPProvider: {}", why),
            //     };
            //     (
            //         osu::get_stars(pp_provider.stars()),
            //         osu::get_pp(score, &pp_provider),
            //     )
            // };
            // let mut combo = format!("**{}x**/", score.max_combo);
            // match score.map.max_combo {
            //     Some(amount) => {
            //         let _ = write!(combo, "{}x", amount);
            //     }
            //     None => combo.push('-'),
            // }
            // let _ = writeln!(
            //     description,
            //     "**{idx}. [#{rank}] [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
            //     {grade} {pp} ~ ({acc}%) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
            //     idx = idx,
            //     rank = score.position,
            //     title = score.map.title,
            //     version = score.map.version,
            //     base = HOMEPAGE,
            //     id = score.map.beatmap_id,
            //     mods = osu::get_mods(score.enabled_mods),
            //     stars = stars,
            //     grade = grade,
            //     pp = pp,
            //     acc = score.accuracy,
            //     score = with_comma_u64(score.score as u64),
            //     combo = combo,
            //     hits = osu::get_hits(&score, mode),
            //     ago = how_long_ago(&score.date)
            // );
        }
        Ok(Self {
            description,
            author: osu::get_user_author(&user),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
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
}
