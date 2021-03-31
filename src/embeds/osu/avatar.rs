use crate::{
    embeds::Author,
    util::constants::{AVATAR_URL, OSU_BASE},
};

use rosu_v2::model::user::User;

pub struct AvatarEmbed {
    author: Author,
    image: String,
    url: String,
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
            author,
            image: format!("{}{}", AVATAR_URL, user.user_id),
            url: format!("{}u/{}", OSU_BASE, user.user_id),
        }
    }
}

impl_builder!(AvatarEmbed { author, image, url });
