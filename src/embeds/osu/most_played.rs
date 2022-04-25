
use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::prelude::{MostPlayedMap, User};

use crate::{
    embeds::osu,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
    },
};

#[derive(EmbedData)]
pub struct MostPlayedEmbed {
    description: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MostPlayedEmbed {
    pub fn new<'m, M>(user: &User, maps: M, pages: (usize, usize)) -> Self
    where
        M: Iterator<Item = &'m MostPlayedMap>,
    {
        let thumbnail = user.avatar_url.to_owned();
        let mut description = String::with_capacity(10 * 70);

        for most_played in maps {
            let map = &most_played.map;
            let mapset = &most_played.mapset;

            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({base}b/{map_id}) [{stars}]",
                count = most_played.count,
                title = mapset.title,
                artist = mapset.artist,
                version = map.version,
                base = OSU_BASE,
                map_id = map.map_id,
                stars = osu::get_stars(map.stars),
            );
        }

        Self {
            thumbnail,
            description,
            title: "Most played maps:",
            author: author!(user),
            footer: FooterBuilder::new(format!("Page {}/{}", pages.0, pages.1)),
        }
    }
}