use std::{borrow::Cow, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_model::{rosu_v2::user::User, SnipeRecent};
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::round, AuthorBuilder, CowUtils,
    FooterBuilder, IntHasher,
};
use eyre::{Result, WrapErr};
use hashbrown::{hash_map::Entry, HashMap};
use rosu_v2::prelude::GameMode;

use super::ModsFormatter;
use crate::{
    commands::osu::Difference, core::Context, manager::redis::RedisData, pagination::Pages,
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
        user: &RedisData<User>,
        diff: Difference,
        scores: &[SnipeRecent],
        pages: &Pages,
        star_map: &mut HashMap<u32, f32, IntHasher>,
        ctx: &Context,
    ) -> Result<Self> {
        let mut description = String::with_capacity(512);

        // not necessary but less ugly than the iterator
        #[allow(clippy::needless_range_loop)]
        for idx in pages.index()..scores.len().min(pages.index() + 5) {
            let score = &scores[idx];

            let stars = match score.stars {
                Some(stars) => *star_map
                    .entry(score.map_id)
                    .and_modify(|entry| *entry = stars)
                    .or_insert(stars),
                None => match star_map.entry(score.map_id) {
                    Entry::Occupied(e) => *e.get(),
                    Entry::Vacant(e) => {
                        let map = ctx
                            .osu_map()
                            .pp_map(score.map_id)
                            .await
                            .wrap_err("failed to get pp map")?;

                        let stars = ctx
                            .pp_parsed(&map, score.map_id, false, GameMode::Osu)
                            .difficulty()
                            .await
                            .stars();

                        *e.insert(stars as f32)
                    }
                },
            };

            let mods = score.mods.as_ref().map(Cow::Borrowed).unwrap_or_default();

            let _ = write!(
                description,
                "**{idx}. [{artist} - {title} [{version}]]({OSU_BASE}b/{id}) {mods}**\n[{stars:.2}★] ~ ({acc}%) ~ ",
                idx = idx + 1,
                artist = score.artist.cow_escape_markdown(),
                title = score.title.cow_escape_markdown(),
                version = score.version.cow_escape_markdown(),
                id = score.map_id,
                mods = ModsFormatter::new(mods.as_ref()),
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
                Difference::Loss => match score.sniper.as_deref() {
                    // should technically always be `Some` but huismetbenen is bugged
                    Some(name) => write!(
                        description,
                        "Sniped by [{name}]({OSU_BASE}u/{user_id}) ",
                        name = name.cow_escape_markdown(),
                        user_id = score.sniper_id,
                    ),
                    None => write!(
                        description,
                        "Sniped by [<unknown user>]({OSU_BASE}u/{})",
                        score.sniper_id
                    ),
                },
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
