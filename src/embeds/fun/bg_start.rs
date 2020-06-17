use crate::embeds::EmbedData;

#[derive(Clone)]
pub struct BGStartEmbed {
    title: String,
    description: String,
}

impl BGStartEmbed {
    pub fn new() -> Self {
        let title = "React to include tag, unreact to exclude tag".to_string();
        let description = "\
        ```\n\
        🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
        🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Log in\n\
        🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old\n\
        ```"
        .to_owned();
        Self { title, description }
    }
}

impl EmbedData for BGStartEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
