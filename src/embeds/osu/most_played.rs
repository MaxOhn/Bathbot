use crate::{
    custom_client::MostPlayedMap,
    embeds::{osu, Author, EmbedData, Footer},
    util::constants::{AVATAR_URL, OSU_BASE},
};

use twilight_embed_builder::image_source::ImageSource;
use rosu::models::User;
use std::fmt::Write;

#[derive(Clone)]
pub struct MostPlayedEmbed {
    description: String,
    author: Author,
    footer: Footer,
    thumbnail: ImageSource,
    title: &'static str,
}

impl MostPlayedEmbed {
    pub fn new<'m, M>(user: &User, maps: M, pages: (usize, usize)) -> Self
    where
        M: Iterator<Item = &'m MostPlayedMap>,
    {
        let thumbnail = ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap();
        let mut description = String::with_capacity(10 * 70);
        for map in maps {
            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({base}b/{map_id}) [{stars}]",
                count = map.count,
                title = map.title,
                artist = map.artist,
                version = map.version,
                base = OSU_BASE,
                map_id = map.beatmap_id,
                stars = osu::get_stars(map.stars),
            );
        }
        Self {
            thumbnail,
            description,
            title: "Most played maps:",
            author: osu::get_user_author(user),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
        }
    }
}

impl EmbedData for MostPlayedEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
}
