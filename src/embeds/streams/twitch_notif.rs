use crate::{
    embeds::{Author, EmbedData},
    twitch::{TwitchStream, TwitchUser},
    util::constants::TWITCH_BASE,
};

#[derive(Clone)]
pub struct TwitchNotifEmbed {
    description: String,
    thumbnail: String,
    image: String,
    title: String,
    url: String,
    author: Author,
}

impl TwitchNotifEmbed {
    pub fn new(stream: &TwitchStream, user: &TwitchUser) -> Self {
        Self {
            title: stream.username.clone(),
            description: stream.title.clone(),
            thumbnail: user.image_url.clone(),
            image: stream.thumbnail_url.clone(),
            url: format!("{}{}", TWITCH_BASE, stream.username),
            author: Author::new("Now live on twitch:".to_string()),
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
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn image(&self) -> Option<&str> {
        Some(&self.image)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
}
