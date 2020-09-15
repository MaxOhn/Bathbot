use crate::{embeds::EmbedData, util::constants::AVATAR_URL};

use rosu::models::User;
use twilight_embed_builder::image_source::ImageSource;

pub struct AvatarEmbed {
    title: String,
    url: String,
    image: ImageSource,
}

impl AvatarEmbed {
    pub fn new(user: User) -> Self {
        let title_text = format!("{}'s osu! avatar:", user.username);
        let url = format!("{}{}", AVATAR_URL, user.user_id);
        Self {
            title: title_text,
            image: ImageSource::url(&url).unwrap(),
            url,
        }
    }
}

impl EmbedData for AvatarEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
}
