use crate::{
    commands::osu::Difference,
    custom_client::SnipeRecent,
    embeds::{osu, Author, Footer},
    error::PPError,
    util::{
        constants::OSU_BASE, datetime::how_long_ago_dynamic, numbers::round,
        osu::prepare_beatmap_file,
    },
    BotResult,
};

use hashbrown::HashMap;
use rosu_pp::{Beatmap, BeatmapExt};
use rosu_v2::model::user::User;
use std::fmt::Write;

pub struct SnipedDiffEmbed {
    description: String,
    thumbnail: String,
    title: &'static str,
    author: Author,
    footer: Footer,
}

impl SnipedDiffEmbed {
    pub async fn new(
        user: &User,
        diff: Difference,
        scores: &[SnipeRecent],
        start: usize,
        pages: (usize, usize),
        maps: &mut HashMap<u32, Beatmap>,
    ) -> BotResult<Self> {
        let mut description = String::with_capacity(512);

        #[allow(clippy::needless_range_loop)]
        for idx in start..scores.len().min(start + 5) {
            let score = &scores[idx];

            let stars = match score.stars {
                Some(stars) => stars,
                None => {
                    #[allow(clippy::map_entry)]
                    if !maps.contains_key(&score.beatmap_id) {
                        let map_path = prepare_beatmap_file(score.beatmap_id).await?;
                        let map = Beatmap::from_path(map_path).await.map_err(PPError::from)?;

                        maps.insert(score.beatmap_id, map);
                    }

                    let map = maps.get(&score.beatmap_id).unwrap();

                    map.stars(score.mods.bits(), None).stars() as f32
                }
            };

            let _ = write!(
                description,
                "**{idx}. [{map}]({base}b/{id}) {mods}**\n[{stars}] ~ ({acc}%) ~ ",
                idx = idx + 1,
                map = score.map,
                base = OSU_BASE,
                id = score.beatmap_id,
                mods = osu::get_mods(score.mods),
                stars = osu::get_stars(stars),
                acc = round(100.0 * score.accuracy),
            );

            let _ = match diff {
                Difference::Gain => match score.sniped {
                    Some(ref name) => write!(
                        description,
                        "Sniped [{name}]({base}u/{id}) ",
                        name = name,
                        base = OSU_BASE,
                        id = score.sniped_id.unwrap_or(2),
                    ),
                    None => write!(description, "Unclaimed until "),
                },
                Difference::Loss => write!(
                    description,
                    "Sniped by [{name}]({base}u/{id}) ",
                    name = score.sniper,
                    base = OSU_BASE,
                    id = score.sniper_id,
                ),
            };

            let _ = write!(description, "{}", how_long_ago_dynamic(&score.date));
            description.push('\n');
        }

        description.pop();

        let title = match diff {
            Difference::Gain => "New national #1s since last week",
            Difference::Loss => "Lost national #1s since last week",
        };

        let footer = Footer::new(format!(
            "Page {}/{} ~ Total: {}",
            pages.0,
            pages.1,
            scores.len()
        ));

        Ok(Self {
            title,
            description,
            author: author!(user),
            thumbnail: user.avatar_url.to_owned(),
            footer,
        })
    }
}

impl_builder!(SnipedDiffEmbed {
    author,
    description,
    footer,
    thumbnail,
    title,
});
