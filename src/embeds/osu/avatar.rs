use crate::{
    embeds::{Author, EmbedData},
    util::constants::{AVATAR_URL, OSU_BASE},
};

use rosu_v2::model::user::User;
use twilight_embed_builder::image_source::ImageSource;

pub struct AvatarEmbed {
    author: Option<Author>,
    url: Option<String>,
    image: Option<ImageSource>,
}

impl AvatarEmbed {
    pub fn new(user: User) -> Self {
        let author = Author::new(&user.username)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, &user.country_code
            ));

        Self {
            author: Some(author),
            url: Some(format!("{}u/{}", OSU_BASE, user.user_id)),
            image: Some(ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap()),
        }
    }
}

impl EmbedData for AvatarEmbed {
    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }
    fn url_owned(&mut self) -> Option<String> {
        self.url.take()
    }
    fn image_owned(&mut self) -> Option<ImageSource> {
        self.image.take()
    }
}
