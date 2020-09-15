use crate::{
    embeds::{Author, EmbedData},
    twitch::{TwitchStream, TwitchUser},
    util::constants::TWITCH_BASE,
};

use twilight_embed_builder::image_source::ImageSource;

pub struct TwitchNotifEmbed {
    description: String,
    thumbnail: ImageSource,
    image: ImageSource,
    title: String,
    url: String,
    author: Author,
}

impl TwitchNotifEmbed {
    pub fn new(stream: &TwitchStream, user: &TwitchUser) -> Self {
        Self {
            title: stream.username.clone(),
            description: stream.title.clone(),
            thumbnail: ImageSource::url(&user.image_url).unwrap(),
            image: ImageSource::url(&stream.thumbnail_url).unwrap(),
            url: format!("{}{}", TWITCH_BASE, stream.username),
            author: Author::new("Now live on twitch:"),
        }
    }
}

impl EmbedData for TwitchNotifEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
}
