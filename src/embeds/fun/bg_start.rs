use crate::embeds::EmbedData;

use twilight_model::id::UserId;

#[derive(Clone)]
pub struct BGStartEmbed {
    description: String,
}

impl BGStartEmbed {
    pub fn new(author: UserId) -> Self {
        let description = format!(
            "**React to include tag, unreact to exclude tag.**\n\
            <@{}> react with ✅ when you're ready.\n\
            (Not all backgrounds have been tagged yet, \
            I suggest to ✅ right away for now)\n\
            ```\n\
            🍋: Easy 🎨: Weeb 😱: Hard name 🗽: English 💯: Tech\n\
            🤓: Hard 🍨: Kpop 🪀: Alternate 🌀: Streams ✅: Lock in\n\
            🤡: Meme 👨‍🌾: Farm 🟦: Blue sky  👴: Old     ❌: Abort\n\
            ```",
            author
        );
        Self { description }
    }
}

impl EmbedData for BGStartEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
