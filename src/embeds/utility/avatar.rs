use crate::{commands::utility::AvatarUser, embeds::EmbedData};

#[derive(Clone)]
pub struct AvatarEmbed {
    title: String,
    url: String,
    image: String,
}

impl AvatarEmbed {
    pub fn new(user: AvatarUser) -> Self {
        let title_text = format!(
            "{}'s {} avatar:",
            user.name(),
            if let AvatarUser::Discord { .. } = user {
                "discord"
            } else {
                "osu!"
            }
        );
        Self {
            title: title_text,
            url: user.url().to_string(),
            image: user.url().to_string(),
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
