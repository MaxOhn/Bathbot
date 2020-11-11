use crate::{
    embeds::EmbedData,
    util::constants::{AVATAR_URL, OSU_BASE},
};

use rosu::model::User;
use twilight_embed_builder::image_source::ImageSource;

pub struct AvatarEmbed {
    title: String,
    url: String,
    image: ImageSource,
}

impl AvatarEmbed {
    pub fn new(user: User) -> Self {
        Self {
            url: format!("{}u/{}", OSU_BASE, user.user_id),
            title: format!("{}'s osu! avatar:", user.username),
            image: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
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
