use crate::{
    embeds::{Author, EmbedData},
    util::constants::{AVATAR_URL, OSU_BASE},
};

use rosu::model::User;
use twilight_embed_builder::image_source::ImageSource;

pub struct AvatarEmbed {
    author: Author,
    url: String,
    image: ImageSource,
}

impl AvatarEmbed {
    pub fn new(user: User) -> Self {
        let author = Author::new(&user.username)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(format!("{}/images/flags/{}.png", OSU_BASE, &user.country));
        Self {
            author,
            url: format!("{}u/{}", OSU_BASE, user.user_id),
            image: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for AvatarEmbed {
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
}
