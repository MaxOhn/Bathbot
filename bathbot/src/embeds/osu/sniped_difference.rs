use std::fmt::Write;

use bathbot_macros::EmbedData;
use eyre::{Result, WrapErr};
use hashbrown::HashMap;
use rosu_pp::{Beatmap, BeatmapExt};

use crate::{
    commands::osu::Difference,
    core::Context,
    custom_client::SnipeRecent,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::HowLongAgoDynamic,
        hasher::IntHasher,
        numbers::round,
        osu::prepare_beatmap_file,
        CowUtils,
    },
};

use super::ModsFormatter;

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
        user: &RedisData<User>,
        diff: Difference,
        scores: &[SnipeRecent],
        pages: &Pages,
        maps: &mut HashMap<u32, Beatmap, IntHasher>,
        ctx: &Context,
    ) -> Result<Self> {
        let mut description = String::with_capacity(512);

        #[allow(clippy::needless_range_loop)]
        for idx in pages.index..scores.len().min(pages.index + 5) {
            let score = &scores[idx];

            let stars = match score.stars {
                Some(stars) => stars,
                None => {
                    #[allow(clippy::map_entry)]
                    if !maps.contains_key(&score.map_id) {
                        let map_path = prepare_beatmap_file(ctx, score.map_id)
                            .await
                            .wrap_err("failed to prepare map")?;

                        let map = Beatmap::from_path(map_path)
                            .await
                            .wrap_err("failed to parse map")?;

                        maps.insert(score.map_id, map);
                    }

                    let map = maps.get(&score.map_id).unwrap();

                    map.stars()
                        .mods(score.mods.unwrap_or_default().bits())
                        .calculate()
                        .stars() as f32
                }
            };

            let _ = write!(
                description,
                "**{idx}. [{artist} - {title} [{version}]]({OSU_BASE}b/{id}) {mods}**\n[{stars:.2}★] ~ ({acc}%) ~ ",
                idx = idx + 1,
                artist = score.artist.cow_escape_markdown(),
                title = score.title.cow_escape_markdown(),
                version = score.version.cow_escape_markdown(),
                id = score.map_id,
                mods = ModsFormatter::new(score.mods.unwrap_or_default()),
                acc = round(score.accuracy),
            );

            let _ = match diff {
                Difference::Gain => match score.sniped.as_deref().zip(score.sniped_id) {
                    Some((name, user_id)) => write!(
                        description,
                        "Sniped [{name}]({OSU_BASE}u/{user_id}) ",
                        name = name.cow_escape_markdown(),
                    ),
                    None => write!(description, "Unclaimed until "),
                },
                Difference::Loss => {
                    write!(
                        description,
                        "Sniped by [{name}]({OSU_BASE}u/{user_id}) ",
                        name = score.sniper.as_str().cow_escape_markdown(),
                        user_id = score.sniper_id,
                    )
                }
            };

            if let Some(ref date) = score.date {
                let _ = write!(description, "{}", HowLongAgoDynamic::new(date));
            } else {
                description.push_str("<unknown date>");
            }

            description.push('\n');
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
            author: user.author_builder(),
            thumbnail: user.avatar_url().to_owned(),
            footer,
        })
    }
}
