use crate::{
    custom_client::{TwitchStream, TwitchUser},
    embeds::Author,
    util::constants::TWITCH_BASE,
};

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
            url: format!("{TWITCH_BASE}{}", user.display_name),
            author: Author::new("Now live on twitch:"),
        }
    }
}

impl_builder!(&TwitchNotifEmbed {
    author,
    description,
    image,
    thumbnail,
    title,
    url,
});
