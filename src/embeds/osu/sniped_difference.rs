use std::fmt::Write;

use command_macros::EmbedData;
use hashbrown::HashMap;
use rosu_pp::{Beatmap, BeatmapExt};
use rosu_v2::model::user::User;

use crate::{
    commands::osu::Difference,
    core::Context,
    custom_client::SnipeRecent,
    embeds::osu,
    error::PpError,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        hasher::SimpleBuildHasher,
        numbers::round,
        osu::prepare_beatmap_file,
        CowUtils,
    },
    BotResult,
};

#[derive(EmbedData)]
pub struct SnipedDiffEmbed {
    description: String,
    thumbnail: String,
    title: &'static str,
    author: AuthorBuilder,
    footer: FooterBuilder,
}

impl SnipedDiffEmbed {
    pub async fn new(
        user: &User,
        diff: Difference,
        scores: &[SnipeRecent],
        pages: &Pages,
        maps: &mut HashMap<u32, Beatmap, SimpleBuildHasher>,
        ctx: &Context,
    ) -> BotResult<Self> {
        let mut description = String::with_capacity(512);

        #[allow(clippy::needless_range_loop)]
        for idx in pages.index..scores.len().min(pages.index + 5) {
            let score = &scores[idx];

            let stars = match score.stars {
                Some(stars) => stars,
                None => {
                    #[allow(clippy::map_entry)]
                    if !maps.contains_key(&score.map_id) {
                        let map_path = prepare_beatmap_file(ctx, score.map_id).await?;
                        let map = Beatmap::from_path(map_path).await.map_err(PpError::from)?;

                        maps.insert(score.map_id, map);
                    }

                    let map = maps.get(&score.map_id).unwrap();

                    map.stars().mods(score.mods.bits()).calculate().stars() as f32
                }
            };

            let _ = write!(
                description,
                "**{idx}. [{map}]({OSU_BASE}b/{id}) {mods}**\n[{stars:.2}★] ~ ({acc}%) ~ ",
                idx = idx + 1,
                map = score.map.cow_escape_markdown(),
                id = score.map_id,
                mods = osu::get_mods(score.mods),
                acc = round(score.accuracy),
            );

            let _ = match diff {
                Difference::Gain => match score.sniped {
                    Some(ref name) => write!(
                        description,
                        "Sniped [{name}]({OSU_BASE}u/{id}) ",
                        name = name.cow_escape_markdown(),
                        id = score.sniped_id.unwrap_or(2),
                    ),
                    None => write!(description, "Unclaimed until "),
                },
                Difference::Loss => write!(
                    description,
                    "Sniped by [{name}]({OSU_BASE}u/{id}) ",
                    name = score.sniper.cow_escape_markdown(),
                    id = score.sniper_id,
                ),
            };

            let _ = writeln!(description, "{}", how_long_ago_dynamic(&score.date));
        }

        description.pop();

        let title = match diff {
            Difference::Gain => "New national #1s since last week",
            Difference::Loss => "Lost national #1s since last week",
        };

        let footer = FooterBuilder::new(format!(
            "Page {}/{} ~ Total: {}",
            pages.curr_page(),
            pages.last_page(),
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
