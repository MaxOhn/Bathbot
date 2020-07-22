use crate::{embeds::EmbedData, util::constants::AVATAR_URL};

use rosu::models::User;

#[derive(Clone)]
pub struct AvatarEmbed {
    title: String,
    url: String,
    image: String,
}

impl AvatarEmbed {
    pub fn new(user: User) -> Self {
        let title_text = "{}'s osu! avatar:".to_owned();
        let url = format!("{}{}", AVATAR_URL, user.user_id);
        Self {
            title: title_text,
            image: url.clone(),
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
    fn image(&self) -> Option<&str> {
        Some(&self.image)
    }
}
