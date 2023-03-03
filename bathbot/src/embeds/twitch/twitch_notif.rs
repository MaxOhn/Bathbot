use bathbot_macros::EmbedData;
use bathbot_model::{TwitchStream, TwitchUser};
use bathbot_util::{constants::TWITCH_BASE, AuthorBuilder};

#[derive(Clone, EmbedData)]
pub struct TwitchNotifEmbed {
    description: String,
    thumbnail: String,
    image: String,
    title: String,
    url: String,
    author: AuthorBuilder,
}

impl TwitchNotifEmbed {
    pub fn new(stream: &TwitchStream, user: &TwitchUser) -> Self {
        Self {
            title: stream.username.to_string(),
            description: stream.title.to_string(),
            thumbnail: user.image_url.to_string(),
            image: stream.thumbnail_url.to_string(),
            url: format!("{TWITCH_BASE}{}", user.display_name),
            author: AuthorBuilder::new("Now live on twitch:"),
        }
    }
}
