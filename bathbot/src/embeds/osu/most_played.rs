use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{constants::OSU_BASE, AuthorBuilder, CowUtils, FooterBuilder};
use rosu_v2::prelude::MostPlayedMap;

use crate::{manager::redis::RedisData, pagination::Pages};

#[derive(EmbedData)]
pub struct MostPlayedEmbed {
    description: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MostPlayedEmbed {
    pub fn new(user: &RedisData<User>, maps: &[MostPlayedMap], pages: &Pages) -> Self {
        let mut description = String::with_capacity(10 * 70);

        for most_played in maps {
            let map = &most_played.map;
            let mapset = &most_played.mapset;

            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id}) [{stars:.2}★]",
                count = most_played.count,
                title = mapset.title.cow_escape_markdown(),
                artist = mapset.artist.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                map_id = map.map_id,
                stars = map.stars,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        Self {
            author: user.author_builder(),
            description,
            footer: FooterBuilder::new(format!("Page {page}/{pages}")),
            thumbnail: user.avatar_url().to_owned(),
            title: "Most played maps:",
        }
    }
}
